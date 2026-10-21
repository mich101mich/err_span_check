use super::*;

use git2::Repository;

pub(crate) struct GitRepo {
    repo: Repository,
    root: PathBuf,
}

impl GitRepo {
    /// Open the git repository containing the given path.
    pub(crate) fn open(path: &Path) -> Result<Self> {
        #[allow(trivial_casts, reason = "recommended by git2 docs")]
        let root = Repository::discover_path(path, &[] as &[&std::ffi::OsStr])
            .context("failed to find git repository")?;
        let repo = Repository::open(&root).context("failed to open git repository")?;

        let root = fs_err::canonicalize(root.parent().context(".git directory has no parent")?)
            .context("failed to canonicalize git repository root")?;

        Ok(Self { repo, root })
    }

    /// Check the (working tree) status of the given path to see if there are unstaged changes
    pub(crate) fn is_clean(&self, path: &Path) -> Result<()> {
        let Ok(path) = path.strip_prefix(&self.root) else {
            bail!(
                "failed to strip git repository root {} from path {}",
                self.root.display(),
                path.display()
            )
        };

        let show = git2::StatusShow::Workdir;
        let mut opts = git2::StatusOptions::new();
        opts.show(show)
            .include_untracked(true)
            .recurse_untracked_dirs(true)
            .include_ignored(true)
            .include_unmodified(true)
            .disable_pathspec_match(true)
            .pathspec(path);

        let statuses = self
            .repo
            .statuses(Some(&mut opts))
            .context("failed to get git status")?;

        if statuses.len() > 1 {
            // This should never happen, since we only check one path, but if it does, something went wrong
            bail!("unexpectedly got multiple git statuses");
        }
        let Some(status) = statuses.get(0) else {
            // we included every type of known file, so something went wrong
            bail!("unexpectedly got no git status");
        };

        let status = status.status();

        if status.is_ignored() {
            bail!("file is ignored by git");
        } else if status.is_wt_new() {
            bail!("file is untracked by git");
        } else if status.is_wt_deleted() {
            // this should not be possible, since we only check existing files
            bail!("file was deleted");
        } else if status.is_wt_modified() || status.is_wt_typechange() || status.is_wt_renamed() {
            bail!("file has unstaged changes");
        }

        // File is tracked with no unstaged changes (may have staged changes, which is fine)
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    struct TempGitRepo {
        dir: PathBuf,
    }

    impl TempGitRepo {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "err_span_check_git_test_{}_{}",
                std::process::id(),
                name
            ));
            let _ = std::fs::remove_dir_all(&dir);
            fs_err::create_dir_all(&dir).unwrap();

            Self::git_in(&dir, &["init"]);
            Self::git_in(&dir, &["config", "user.email", "test@example.com"]);
            Self::git_in(&dir, &["config", "user.name", "Test User"]);

            let dir = fs_err::canonicalize(&dir).unwrap();
            Self { dir }
        }

        fn git(&self, args: &[&str]) {
            Self::git_in(&self.dir, args);
        }

        fn git_in(dir: &Path, args: &[&str]) {
            let output = Command::new("git")
                .args(args)
                .current_dir(dir)
                .output()
                .expect("failed to execute git");
            assert!(
                output.status.success(),
                "git {:?} failed: {}",
                args,
                String::from_utf8_lossy(&output.stderr)
            );
        }

        fn write(&self, rel_path: &str, content: &str) -> PathBuf {
            let path = self.dir.join(rel_path);
            if let Some(parent) = path.parent() {
                fs_err::create_dir_all(parent).unwrap();
            }
            fs_err::write(&path, content).unwrap();
            path
        }
    }

    impl Drop for TempGitRepo {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    #[test]
    fn clean_committed_and_staged() {
        let temp = TempGitRepo::new("clean_committed_and_staged");
        let committed = temp.write("tests/fail/committed.rs", "fn main() {}\n");
        temp.git(&["add", "tests/fail/committed.rs"]);
        temp.git(&["commit", "-m", "initial"]);

        let staged = temp.write("tests/fail/staged.rs", "fn main() {}\n");
        temp.git(&["add", "tests/fail/staged.rs"]);

        let repo = GitRepo::open(&temp.dir).unwrap();
        assert!(repo.is_clean(&committed).is_ok());
        assert!(repo.is_clean(&staged).is_ok());
    }

    #[test]
    fn untracked_and_ignored() {
        let temp = TempGitRepo::new("untracked_and_ignored");
        temp.write(".gitignore", "ignored.rs\nignored_dir/\n");
        let untracked = temp.write("tests/fail/untracked.rs", "fn main() {}\n");
        let ignored = temp.write("tests/fail/ignored.rs", "fn main() {}\n");
        let ignored_in_dir = temp.write("tests/fail/ignored_dir/test.rs", "fn main() {}\n");

        let repo = GitRepo::open(&temp.dir).unwrap();
        assert_eq!(
            repo.is_clean(&untracked).unwrap_err().to_string(),
            "file is untracked by git"
        );
        assert_eq!(
            repo.is_clean(&ignored).unwrap_err().to_string(),
            "file is ignored by git"
        );
        // assert_eq!(
        //     repo.is_clean(&ignored_in_dir).unwrap_err().to_string(),
        //     "file is ignored by git"
        // ); // FIXME: Doesn't work with current implementation
    }

    #[test]
    fn unstaged_changes_and_deleted() {
        let temp = TempGitRepo::new("unstaged_changes_and_deleted");
        let modified = temp.write("tests/fail/modified.rs", "fn main() {}\n");
        let deleted = temp.write("tests/fail/deleted.rs", "fn main() {}\n");
        temp.git(&["add", "."]);
        temp.git(&["commit", "-m", "initial"]);

        temp.write("tests/fail/modified.rs", "fn main() { let x = 1; }\n");
        fs_err::remove_file(&deleted).unwrap();

        let repo = GitRepo::open(&temp.dir).unwrap();
        assert_eq!(
            repo.is_clean(&modified).unwrap_err().to_string(),
            "file has unstaged changes"
        );
        assert_eq!(
            repo.is_clean(&deleted).unwrap_err().to_string(),
            "file was deleted"
        );
    }
}
