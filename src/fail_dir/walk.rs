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

    let repo = git::GitRepo::open(&fail_dir)?;
    let dir = Directory::scan(fail_dir, PathBuf::new(), &repo)?;

    let is_nightly = rustc_version::version_meta()
        .is_ok_and(|meta| meta.channel == rustc_version::Channel::Nightly);
    let mut state = FailDirState::new(is_nightly);
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
    git_status: Result<()>,
}

struct FailDirState {
    /// Maps file stems to the number of times they have been used so far. This allows us to generate unique filenames for test cases.
    unique_files: HashMap<String, usize>,
    /// Flag if we are using the nightly channel
    is_nightly: bool,
    /// List of all test files found in the fail directory
    test_files: Vec<TestFile>,
}

impl FailDirState {
    fn new(is_nightly: bool) -> Self {
        Self {
            unique_files: HashMap::new(),
            is_nightly,
            test_files: vec![],
        }
    }

    fn add_test_file(&mut self, mut file: File, stem: &str) {
        let git_status = std::mem::replace(&mut file.git_status, Ok(()));

        let mut file = self.make_test_file(file, stem);

        // parsing error takes precedence over git error, since it means the file will need to be modified anyway
        if file.error.is_none()
            && let Err(git_error) = git_status
        {
            file.git_status = Err(git_error);
        }

        self.test_files.push(file);
    }

    fn make_test_file(&mut self, file: File, stem: &str) -> TestFile {
        // Ensure unique filenames by appending a counter
        let count = self.unique_files.entry(stem.to_string()).or_insert(0);
        *count += 1;
        let stem = format!("{}_{}", stem, count);

        TestFile::from_file(file.path, file.relative_path, &stem, file.content)
    }

    fn add_test_error(&mut self, path: PathBuf, relative_path: PathBuf, error: Error) {
        let test_file = TestFile::from_error(path, relative_path, error);
        self.test_files.push(test_file);
    }
}

impl Directory {
    fn scan(dir: PathBuf, relative_dir: PathBuf, repo: &git::GitRepo) -> Result<Self> {
        let mut files = BTreeMap::new();
        let mut subdirs = BTreeMap::new();

        for entry in
            std::fs::read_dir(&dir).path_context(&dir, "failed to read directory: <path>")?
        {
            let entry = entry.path_context(&dir, "failed to read directory entry in <path>")?;

            let filename = entry.file_name();
            let path = entry.path();
            let relative_path = relative_dir.join(&filename);
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

                if stem == "main" && relative_dir.as_os_str().is_empty() {
                    bail!(
                        r#"filename 'main.rs' is not allowed in the root of the fail directory, as it would cause conflicts with Rust's test runner.
Please choose a different name for the test file: {}"#,
                        path.display()
                    );
                }

                let path = path.canonicalize().unwrap_or_else(|_| path.clone());

                let content = std::fs::read_to_string(&path)
                    .path_context(&path, "failed to read test file: <path>")?;
                let git_status = repo.is_clean(&path);

                files.insert(
                    stem.to_owned(),
                    File {
                        path,
                        relative_path,
                        content,
                        git_status,
                    },
                );
            } else if path.is_dir() {
                let subdir = Directory::scan(path, relative_path, repo)?;
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
                nightly_subdir.dir.display()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn s(t: &str) -> String {
        t.to_string()
    }
    fn p(t: impl AsRef<str>) -> PathBuf {
        PathBuf::from(t.as_ref())
    }
    fn to_map<V, const N: usize>(pairs: [(&str, V); N]) -> BTreeMap<String, V> {
        pairs.into_iter().map(|(k, v)| (s(k), v)).collect()
    }

    mod ord_union {
        use super::*;

        #[test]
        fn basic() {
            let a = to_map([("a", true), ("b", true), ("d", true), ("e", true)]);
            let b = to_map([("b", false), ("c", false), ("d", false), ("f", false)]);

            let result: Vec<_> = OrdUnion::new(a, b).collect();
            assert_eq!(result.len(), 6);
            assert_eq!(result[0], (Some((s("a"), true)), None));
            assert_eq!(result[1], (Some((s("b"), true)), Some((s("b"), false))));
            assert_eq!(result[2], (None, Some((s("c"), false))));
            assert_eq!(result[3], (Some((s("d"), true)), Some((s("d"), false))));
            assert_eq!(result[4], (Some((s("e"), true)), None));
            assert_eq!(result[5], (None, Some((s("f"), false))));
        }

        #[test]
        fn only_left() {
            let a = to_map([("a", true), ("b", true), ("d", true), ("e", true)]);
            let b = to_map([]);

            let result: Vec<_> = OrdUnion::new(a, b).collect();
            assert_eq!(result.len(), 4);
            assert_eq!(result[0], (Some((s("a"), true)), None));
            assert_eq!(result[1], (Some((s("b"), true)), None));
            assert_eq!(result[2], (Some((s("d"), true)), None));
            assert_eq!(result[3], (Some((s("e"), true)), None));
        }

        #[test]
        fn only_right() {
            let a = to_map([]);
            let b = to_map([("b", false), ("c", false), ("d", false), ("f", false)]);

            let result: Vec<_> = OrdUnion::new(a, b).collect();
            assert_eq!(result.len(), 4);
            assert_eq!(result[0], (None, Some((s("b"), false))));
            assert_eq!(result[1], (None, Some((s("c"), false))));
            assert_eq!(result[2], (None, Some((s("d"), false))));
            assert_eq!(result[3], (None, Some((s("f"), false))));
        }

        #[test]
        fn b_then_a() {
            let a = to_map([("x", true), ("y", true), ("z", true)]);
            let b = to_map([("a", false), ("b", false), ("c", false)]);

            let result: Vec<_> = OrdUnion::new(a, b).collect();
            assert_eq!(result.len(), 6);
            assert_eq!(result[0], (None, Some((s("a"), false))));
            assert_eq!(result[1], (None, Some((s("b"), false))));
            assert_eq!(result[2], (None, Some((s("c"), false))));
            assert_eq!(result[3], (Some((s("x"), true)), None));
            assert_eq!(result[4], (Some((s("y"), true)), None));
            assert_eq!(result[5], (Some((s("z"), true)), None));
        }

        #[test]
        fn empty() {
            let a = to_map([]);
            let b = to_map([]);

            let result: Vec<_> = OrdUnion::<bool>::new(a, b).collect();
            assert!(result.is_empty());
        }
    }

    mod flatten_directory {
        use super::*;

        fn file<'a>(
            relative_path: &str,
            name: &'a str,
            content: Result<&str, &str>,
            git_status: Result<(), &str>,
        ) -> (&'a str, File) {
            let content = match content {
                Ok(c) => format!("///// test\n{c}"),
                Err(e) => s(e),
            };
            let git_status = match git_status {
                Ok(()) => Ok(()),
                Err(e) => Err(anyhow::anyhow!(s(e))),
            };
            let relative_path = if relative_path.is_empty() {
                format!("{name}.rs")
            } else {
                format!("{relative_path}/{name}.rs")
            };
            let file = File {
                path: p(format!("/fail/{relative_path}")),
                relative_path: p(relative_path),
                content,
                git_status,
            };
            (name, file)
        }
        fn dir<'a, const F: usize, const D: usize>(
            relative_path: &str,
            name: &'a str,
            files: [(&str, File); F],
            subdirs: [(&str, Directory); D],
        ) -> (&'a str, Directory) {
            let relative_path = if relative_path.is_empty() {
                s(name)
            } else {
                format!("{relative_path}/{name}")
            };
            let dir = Directory {
                dir: p(format!("/fail/{relative_path}")),
                relative_dir: p(relative_path),
                files: to_map(files),
                subdirs: to_map(subdirs),
            };
            (name, dir)
        }

        fn make_basic_dir() -> Directory {
            let a = file("", "a", Ok("test content"), Ok(()));
            let b = file("", "b", Err("faulty content"), Ok(()));
            let c = file("stable", "c", Ok("more content"), Err("git error"));
            let d = file("stable/inner", "d", Ok("inner content"), Ok(()));
            let e = file("other", "e", Ok("other content"), Ok(()));
            Directory {
                dir: p("/fail"),
                relative_dir: p(""),
                files: to_map([a, b]),
                subdirs: to_map([
                    dir("", "stable", [c], [dir("stable", "inner", [d], [])]),
                    dir("", "other", [e], []),
                ]),
            }
        }

        #[test]
        fn basic_stable() {
            let mut state = FailDirState::new(false);
            flatten_directory(make_basic_dir(), &mut state);

            let mut iter = state.test_files.into_iter();
            let a = iter.next().unwrap();
            assert_eq!(a.path, p("/fail/a.rs"));
            assert_eq!(a.relative_path, p("a.rs"));
            assert_eq!(a.original_content, "///// test\ntest content");
            assert!(!a.tests.is_empty());
            assert!(!a.has_error());

            let b = iter.next().unwrap();
            assert_eq!(b.path, p("/fail/b.rs"));
            assert_eq!(b.relative_path, p("b.rs"));
            assert_eq!(b.original_content, ""); // empty due to error
            assert!(b.tests.is_empty()); // no tests due to error
            assert!(b.git_status.is_ok());
            assert_eq!(
                b.error.unwrap().to_string(),
                "Failed to parse test case from /fail/b.rs:1: no test cases found"
            );

            let c = iter.next().unwrap();
            assert_eq!(c.path, p("/fail/stable/c.rs"));
            assert_eq!(c.relative_path, p("stable/c.rs"));
            assert_eq!(c.original_content, "///// test\nmore content");
            assert!(!c.tests.is_empty());
            assert_eq!(c.git_status.unwrap_err().to_string(), "git error");
            assert!(c.error.is_none());

            let d = iter.next().unwrap();
            assert_eq!(d.path, p("/fail/stable/inner/d.rs"));
            assert_eq!(d.relative_path, p("stable/inner/d.rs"));
            assert_eq!(d.original_content, "///// test\ninner content");
            assert!(!d.tests.is_empty());
            assert!(!d.has_error());

            let e = iter.next().unwrap();
            assert_eq!(e.path, p("/fail/other/e.rs"));
            assert_eq!(e.relative_path, p("other/e.rs"));
            assert_eq!(e.original_content, "///// test\nother content");
            assert!(!e.tests.is_empty());
            assert!(!e.has_error());

            assert!(iter.next().is_none());
        }

        #[test]
        fn basic_nightly() {
            let mut state = FailDirState::new(true);
            flatten_directory(make_basic_dir(), &mut state);

            let mut iter = state.test_files.into_iter();
            let a = iter.next().unwrap();
            assert_eq!(a.path, p("/fail/a.rs"));
            assert_eq!(a.relative_path, p("a.rs"));
            assert_eq!(a.original_content, "///// test\ntest content");
            assert!(!a.tests.is_empty());
            assert!(!a.has_error());

            let b = iter.next().unwrap();
            assert_eq!(b.path, p("/fail/b.rs"));
            assert_eq!(b.relative_path, p("b.rs"));
            assert_eq!(b.original_content, ""); // empty due to error
            assert!(b.tests.is_empty()); // no tests due to error
            assert!(b.git_status.is_ok());
            assert_eq!(
                b.error.unwrap().to_string(),
                "Failed to parse test case from /fail/b.rs:1: no test cases found"
            );

            let c = iter.next().unwrap();
            assert_eq!(c.path, p("/fail/nightly/c.rs"));
            assert_eq!(c.relative_path, p("nightly/c.rs"));
            assert_eq!(c.original_content, ""); // file did not exist before
            assert!(!c.tests.is_empty());
            assert!(!c.has_error());

            let d = iter.next().unwrap();
            assert_eq!(d.path, p("/fail/nightly/inner/d.rs"));
            assert_eq!(d.relative_path, p("nightly/inner/d.rs"));
            assert_eq!(d.original_content, ""); // file did not exist before
            assert!(!d.tests.is_empty());
            assert!(!d.has_error());

            let e = iter.next().unwrap();
            assert_eq!(e.path, p("/fail/other/e.rs"));
            assert_eq!(e.relative_path, p("other/e.rs"));
            assert_eq!(e.original_content, "///// test\nother content");
            assert!(!e.tests.is_empty());
            assert!(!e.has_error());

            assert!(iter.next().is_none());
        }

        #[test]
        fn nested_stable_nightly() {
            let a = file("stable/stable", "a", Ok("test content"), Ok(()));
            let dir = Directory {
                dir: p("/fail"),
                relative_dir: p(""),
                files: to_map([]),
                subdirs: to_map([dir("", "stable", [], [dir("stable", "stable", [a], [])])]),
            };

            let mut state = FailDirState::new(false);
            flatten_directory(dir, &mut state);

            let mut iter = state.test_files.into_iter();
            let inner_dir = iter.next().unwrap();
            assert_eq!(inner_dir.path, p("/fail/stable"));
            assert_eq!(inner_dir.relative_path, p("stable"));
            assert!(inner_dir.error.is_some());
            assert_eq!(
                inner_dir.error.as_ref().unwrap().to_string(),
                "nested stable/nightly directories are not allowed"
            );

            assert!(iter.next().is_none());
        }

        #[test]
        fn nightly_without_stable_toplevel() {
            let dir = Directory {
                dir: p("/fail"),
                relative_dir: p(""),
                files: to_map([]),
                subdirs: to_map([dir("", "nightly", [], [])]),
            };

            let mut state = FailDirState::new(true);
            flatten_directory(dir, &mut state);

            let mut iter = state.test_files.into_iter();
            let nightly = iter.next().unwrap();
            assert_eq!(nightly.path, p("/fail/nightly"));
            assert_eq!(nightly.relative_path, p("nightly"));
            assert_eq!(
                nightly.error.as_ref().unwrap().to_string(),
                "found nightly directory in /fail without corresponding stable directory"
            );

            assert!(iter.next().is_none());
        }

        #[test]
        fn nightly_without_stable_inner() {
            let a = file("nightly", "a", Ok("test content"), Ok(()));
            let dir = Directory {
                dir: p("/fail"),
                relative_dir: p(""),
                files: to_map([]),
                subdirs: to_map([
                    dir("", "stable", [], []),
                    dir("", "nightly", [a], [dir("nightly", "inner", [], [])]),
                ]),
            };

            let mut state = FailDirState::new(true);
            flatten_directory(dir, &mut state);

            let mut iter = state.test_files.into_iter();
            let a = iter.next().unwrap();
            assert_eq!(a.path, p("/fail/nightly/a.rs"));
            assert_eq!(a.relative_path, p("nightly/a.rs"));
            assert_eq!(
                a.error.as_ref().unwrap().to_string(),
                "found nightly file /fail/nightly/a.rs without corresponding file in stable directory"
            );

            let inner = iter.next().unwrap();
            assert_eq!(inner.path, p("/fail/nightly/inner"));
            assert_eq!(inner.relative_path, p("nightly/inner"));
            assert_eq!(
                inner.error.as_ref().unwrap().to_string(),
                "found nightly sub-directory /fail/nightly/inner without corresponding sub-directory in stable directory"
            );

            assert!(iter.next().is_none());
        }
    }
}
