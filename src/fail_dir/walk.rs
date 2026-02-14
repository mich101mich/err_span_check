use super::*;

pub(crate) fn parse_test_files(base_dir: PathBuf) -> Result<Vec<TestFile>, Error> {
    let fail_dir = base_dir.join("tests").join("fail");
    if !fail_dir.is_dir() {
        // We could say "no tests found" here, but the user explicitly called this function, so they want to run tests.
        // If the directory is missing, it probably means they set up their project incorrectly.
        bail!(
            "could not find or access tests/fail directory relative to {}",
            base_dir.display()
        );
    }

    let mut state = FailDirState::new(&fail_dir)?;

    let dir = Directory::scan(fail_dir, PathBuf::new())?;
    flatten_directory(dir, &mut state);

    Ok(state.test_files)
}

struct Directory {
    dir: PathBuf,
    relative_dir: PathBuf,
    files: BTreeMap<String, File>,
    subdirs: BTreeMap<String, Directory>,
}

struct File {
    path: PathBuf,
    relative_path: PathBuf,
    content: String,
}

struct FailDirState {
    /// Maps file stems to the number of times they have been used so far. This allows us to generate unique filenames for test cases.
    unique_files: HashMap<String, usize>,
    /// Flag if we are using the nightly channel
    is_nightly: bool,
    /// Git repository for checking if files are clean
    repo: git::GitRepo,
    /// List of all test files found in the fail directory
    test_files: Vec<TestFile>,
}

impl FailDirState {
    fn new(fail_dir: &Path) -> Result<Self, Error> {
        Ok(Self {
            unique_files: HashMap::new(),
            is_nightly: rustc_version::version_meta()
                .is_ok_and(|meta| meta.channel == rustc_version::Channel::Nightly),
            repo: git::GitRepo::open(fail_dir)?,
            test_files: vec![],
        })
    }

    fn add_test_file(&mut self, file: File, stem: &str) {
        let file = self.make_test_file(file, stem);
        self.test_files.push(file);
    }

    fn make_test_file(&mut self, file: File, stem: &str) -> TestFile {
        // Ensure unique filenames by appending a counter
        let count = self.unique_files.entry(stem.to_string()).or_insert(0);
        *count += 1;
        let stem = format!("{}_{}", stem, count);

        TestFile::from_file(
            file.path,
            file.relative_path,
            &stem,
            file.content,
            &self.repo,
        )
    }

    fn add_test_error(&mut self, path: PathBuf, relative_path: PathBuf, error: Error) {
        let test_file = TestFile::from_error(path, relative_path, error);
        self.test_files.push(test_file);
    }
}

impl Directory {
    fn scan(dir: PathBuf, relative_dir: PathBuf) -> Result<Self> {
        let mut files = BTreeMap::new();
        let mut subdirs = BTreeMap::new();

        for entry in
            std::fs::read_dir(&dir).path_context(&dir, "failed to read directory: <path>")?
        {
            let entry = entry.path_context(&dir, "failed to read directory entry in <path>")?;

            let filename = entry.file_name();
            let path = entry.path();
            if path.is_file() {
                let stem = filename
                    .to_str()
                    .and_then(|f| f.strip_suffix(".rs"))
                    .filter(|stem| !stem.is_empty())
                    .path_context(
                        &dir,
                        r#"Invalid filename: <path>. Expected "<valid-nonempty-utf8>.rs
Note that the tests/fail directory is only allowed to contain compile-fail test files."#,
                    )?;

                let content = std::fs::read_to_string(&path)
                    .path_context(&path, "failed to read test file: <path>")?;

                files.insert(
                    stem.to_owned(),
                    File {
                        path: path.clone(),
                        relative_path: relative_dir.join(&filename),
                        content,
                    },
                );
            } else if path.is_dir() {
                let subdir = Directory::scan(path, relative_dir.join(&filename))?;
                if !subdir.files.is_empty() || !subdir.subdirs.is_empty() {
                    let name = filename.to_string_lossy().to_string();
                    subdirs.insert(name, subdir);
                }
            } else {
                bail!(
                    "unexpected entry in fail directory: {} (not a test case or sub-directory)",
                    path.display()
                );
            }
        }

        Ok(Directory {
            dir,
            relative_dir,
            files,
            subdirs,
        })
    }
}

fn flatten_directory(mut dir: Directory, state: &mut FailDirState) {
    for (stem, file) in dir.files {
        state.add_test_file(file, &stem);
    }

    let stable = dir.subdirs.remove("stable");
    let nightly = dir.subdirs.remove("nightly");

    // We want to allow distinction of stable and nightly tests.
    // However, we also want to ensure that both test the same input.
    // So, we use the tests from "stable" as the source of truth, which is then copied to the "nightly" directory.
    if state.is_nightly {
        if let Some(stable) = stable {
            let nightly = nightly.unwrap_or_else(|| Directory {
                dir: dir.dir.join("nightly"),
                relative_dir: dir.relative_dir.join("nightly"),
                files: BTreeMap::new(),
                subdirs: BTreeMap::new(),
            });

            merge_nightly_from_stable(stable, nightly, state);
        } else if let Some(nightly) = nightly {
            let error = anyhow::anyhow!(
                "found nightly directory in {} without corresponding stable directory",
                dir.dir.display()
            );
            state.add_test_error(nightly.dir, nightly.relative_dir, error);
        }
    } else {
        // On stable, we only look at the "stable" directory. "nightly" will be checked when we are on nightly.
        if let Some(stable) = stable {
            flatten_stable_directory(stable, state);
        }
    }

    // process remaining subdirectories that are not stable/nightly
    for subdir in dir.subdirs.into_values() {
        flatten_directory(subdir, state);
    }
}

/// Like `flatten_directory`, but already within a "stable" directory, so no further stable/nightly distinction possible.
fn flatten_stable_directory(stable: Directory, state: &mut FailDirState) {
    if stable.subdirs.contains_key("stable") || stable.subdirs.contains_key("nightly") {
        let error = anyhow::anyhow!("nested stable/nightly directories are not allowed");
        state.add_test_error(stable.dir, stable.relative_dir, error);
        return;
    }

    for (stem, file) in stable.files {
        state.add_test_file(file, &stem);
    }

    for subdir in stable.subdirs.into_values() {
        flatten_stable_directory(subdir, state);
    }
}

/// Walk the nightly directory, using the corresponding stable directory as a reference.
fn merge_nightly_from_stable(stable: Directory, nightly: Directory, state: &mut FailDirState) {
    for (stable_entry, nightly_entry) in OrdUnion::new(stable.files, nightly.files) {
        let Some((stem, stable_file)) = stable_entry else {
            let (_, nightly_file) = nightly_entry.unwrap(); // unwrap: guaranteed by OrdUnion
            let error = anyhow::anyhow!(
                "found nightly file {} without corresponding file in stable directory",
                nightly_file.path.display()
            );
            state.add_test_error(nightly_file.path, nightly_file.relative_path, error);
            continue;
        };

        let original = state.make_test_file(stable_file, &stem);

        let test_file = if let Some((_, nightly_file)) = nightly_entry {
            TestFile::copy_from(
                original,
                nightly_file.path,
                nightly_file.relative_path,
                &stem,
                nightly_file.content,
            )
        } else {
            let filename = format!("{}.rs", stem);
            let content = String::new(); // file did not exist before

            TestFile::copy_from(
                original,
                nightly.dir.join(&filename),
                nightly.relative_dir.join(&filename),
                &stem,
                content,
            )
        };

        state.test_files.push(test_file);
    }

    for (stable_subdir, nightly_subdir) in OrdUnion::new(stable.subdirs, nightly.subdirs) {
        let Some((name, stable_subdir)) = stable_subdir else {
            let (_, nightly_subdir) = nightly_subdir.unwrap(); // unwrap: guaranteed by OrdUnion
            let error = anyhow::anyhow!(
                "found nightly sub-directory {} without corresponding sub-directory in stable directory",
                nightly_subdir.relative_dir.display()
            );
            state.add_test_error(nightly_subdir.dir, nightly_subdir.relative_dir, error);
            continue;
        };

        let nightly_subdir = nightly_subdir
            .map(|(_, dir)| dir)
            .unwrap_or_else(|| Directory {
                dir: nightly.dir.join(&name),
                relative_dir: nightly.relative_dir.join(&name),
                files: BTreeMap::new(),
                subdirs: BTreeMap::new(),
            });

        merge_nightly_from_stable(stable_subdir, nightly_subdir, state);
    }
}

/// An iterator that merges two sorted iterators based on a provided comparison function, yielding pairs of items from
/// both iterators that are considered equal by the comparison function, as well as items that are only present in one
/// of the iterators.
///
/// Similar to `BTreeSet::union`, but will still yield both items even if they are considered equal.
struct OrdUnion<V> {
    a: std::iter::Peekable<std::collections::btree_map::IntoIter<String, V>>,
    b: std::iter::Peekable<std::collections::btree_map::IntoIter<String, V>>,
}

impl<V> OrdUnion<V> {
    fn new(a: BTreeMap<String, V>, b: BTreeMap<String, V>) -> Self {
        Self {
            a: a.into_iter().peekable(),
            b: b.into_iter().peekable(),
        }
    }
}

impl<V> Iterator for OrdUnion<V> {
    type Item = (Option<(String, V)>, Option<(String, V)>);

    fn next(&mut self) -> Option<Self::Item> {
        let (use_a, use_b) = match (self.a.peek(), self.b.peek()) {
            // ideal case: both iterators have the entry
            (Some((a, _)), Some((b, _))) if a == b => (true, true),

            // element in `a` which does not exist in `b`
            (Some((a, _)), Some((b, _))) if a < b => (true, false),
            (Some(_), None) => (true, false),

            // element in `b` which does not exist in `a`
            (Some(_), Some(_)) => (false, true),
            (None, Some(_)) => (false, true),

            (None, None) => return None, // end of both iterators
        };
        let a = self.a.next_if(|_| use_a);
        let b = self.b.next_if(|_| use_b);
        Some((a, b))
    }
}
