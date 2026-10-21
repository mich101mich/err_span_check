use super::*;

use std::process::Command;

/// Working-tree status entries from `git status`, keyed by path relative to the repository root.
pub(crate) struct GitRepo {
    root: PathBuf,
    statuses: HashMap<PathBuf, [u8; 2]>,
}

impl GitRepo {
    /// Run `git status` once for the repository containing the given directory.
    pub(crate) fn open(path: &Path) -> Result<Self> {
        let root = git_output(path, &["rev-parse", "--show-toplevel"])
            .context("failed to find git repository")?;
        let root = String::from_utf8(root).context("git repository root is not valid UTF-8")?;
        let root = fs_err::canonicalize(root.trim_end_matches(['\r', '\n']))
            .context("failed to canonicalize git repository root")?;

        let stdout = git_output(
            path,
            &[
                "status",
                "--porcelain=v1",
                "-z",
                "--no-renames",
                "--ignored=matching", // show ignored files, but not recursively for ignored directories
                "--untracked-files=normal", // show untracked files, but not recursively for untracked directories
                "--",
                ".",
            ],
        )
        .context("failed to get git status")?;

        let mut statuses = HashMap::new();
        for entry in stdout.split(|&b| b == 0).filter(|entry| !entry.is_empty()) {
            let [x, y, b' ', path @ ..] = entry else {
                bail!("unexpected git status entry");
            };
            let path = std::str::from_utf8(path).context("git returned a non-UTF-8 path")?;
            let path = path.trim_end_matches(['/', '\\']); // for ignored/untracked directories
            statuses.insert(PathBuf::from(path), [*x, *y]);
        }

        Ok(Self { root, statuses })
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

        // Paths not listed by `git status` are tracked and unchanged; check ancestors for collapsed directories
        let Some(&[x, y]) = path.ancestors().find_map(|p| self.statuses.get(p)) else {
            return Ok(());
        };

        match [x, y] {
            [b'!', b'!'] => bail!("file is ignored by git"),
            [b'?', b'?'] => bail!("file is untracked by git"),
            [_, b'D'] => bail!("file was deleted"),
            // staged changes are fine, only the working tree matters
            [_, b' '] => Ok(()),
            _ => bail!("file has unstaged changes"),
        }
    }
}

fn git_output(dir: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .context("failed to run git")?;
    if !output.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(output.stdout)
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
        let untracked_in_dir = temp.write("tests/fail/untracked_dir/test.rs", "fn main() {}\n");
        let ignored = temp.write("tests/fail/ignored.rs", "fn main() {}\n");
        let ignored_in_dir = temp.write("tests/fail/ignored_dir/test.rs", "fn main() {}\n");

        let repo = GitRepo::open(&temp.dir).unwrap();
        assert_eq!(
            repo.is_clean(&untracked).unwrap_err().to_string(),
            "file is untracked by git"
        );
        assert_eq!(
            repo.is_clean(&untracked_in_dir).unwrap_err().to_string(),
            "file is untracked by git"
        );
        assert_eq!(
            repo.is_clean(&ignored).unwrap_err().to_string(),
            "file is ignored by git"
        );
        assert_eq!(
            repo.is_clean(&ignored_in_dir).unwrap_err().to_string(),
            "file is ignored by git"
        );
    }

    #[test]
    fn force_added_in_ignored_dir() {
        let temp = TempGitRepo::new("force_added_in_ignored_dir");
        temp.write(".gitignore", "ignored_dir/\n");
        let force_added_committed =
            temp.write("tests/fail/ignored_dir/committed.rs", "fn main() {}\n");
        let force_added_staged = temp.write("tests/fail/ignored_dir/staged.rs", "fn main() {}\n");
        let force_added_modified =
            temp.write("tests/fail/ignored_dir/modified.rs", "fn main() {}\n");
        let untracked_sibling = temp.write("tests/fail/ignored_dir/untracked.rs", "fn main() {}\n");

        temp.git(&["add", "-f", "tests/fail/ignored_dir/committed.rs"]);
        temp.git(&["add", "-f", "tests/fail/ignored_dir/modified.rs"]);
        temp.git(&["commit", "-m", "initial"]);

        temp.git(&["add", "-f", "tests/fail/ignored_dir/staged.rs"]);
        temp.write(
            "tests/fail/ignored_dir/modified.rs",
            "fn main() { let x = 1; }\n",
        );

        let repo = GitRepo::open(&temp.dir).unwrap();
        assert!(repo.is_clean(&force_added_committed).is_ok());
        assert!(repo.is_clean(&force_added_staged).is_ok());
        assert_eq!(
            repo.is_clean(&force_added_modified)
                .unwrap_err()
                .to_string(),
            "file has unstaged changes"
        );
        assert_eq!(
            repo.is_clean(&untracked_sibling).unwrap_err().to_string(),
            "file is ignored by git"
        );
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
