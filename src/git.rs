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
