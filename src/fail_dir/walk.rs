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

    let mut test_files = vec![];

    let is_nightly = rustc_version::version_meta()
        .is_ok_and(|meta| meta.channel == rustc_version::Channel::Nightly);

    let file_iter = walkdir::WalkDir::new(&fail_dir)
        .min_depth(1)
        .follow_links(true)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|e| {
            if !e.file_type().is_dir() {
                return true;
            }
            // If on nightly, skip folders called "stable" and vice versa
            let name = e.file_name().to_string_lossy();
            if is_nightly {
                name != "stable"
            } else {
                name != "nightly"
            }
        });

    let repo = git::GitRepo::open(&fail_dir)?;
    let mut unique_files = HashMap::new();
    for entry in file_iter {
        let entry = entry.context("failed to read directory entry")?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.into_path();

        let stem = path
            .file_name()
            .and_then(|s| s.to_str())
            .and_then(|f| f.strip_suffix(".rs"))
            .filter(|stem| !stem.is_empty())
            .with_context(|| {
                format!(
                    r#"Invalid filename: {path:?}. Expected "<valid-nonempty-utf8>.rs"
Note that the tests/fail directory is only allowed to contain compile-fail test files."#
                )
            })?
            .to_owned();

        // Ensure unique filenames by appending a counter
        let count = unique_files.entry(stem.clone()).or_insert(0);
        *count += 1;
        let stem = format!("{}_{}", stem, count);

        let relative_path = path
            .strip_prefix(&fail_dir)
            .map(ToOwned::to_owned)
            .unwrap_or(path.clone());

        let file = TestFile::from_file(path, relative_path, &stem, &repo);

        test_files.push(file);
    }
    Ok(test_files)
}
