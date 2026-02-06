use crate::*;

pub(crate) fn run() -> Result<()> {
    let base_dir = cargo::manifest_dir()?;
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

    let mut unique_files = HashMap::new();

    for entry in file_iter {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.into_path();
        let parsed_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .and_then(|f| f.rsplit_once('.'))
            .filter(|(stem, ext)| !stem.is_empty() && *ext == "rs")
            .map(|(stem, _)| stem.to_owned());

        // TODO: check git status

        let relative_path = path
            .strip_prefix(&fail_dir)
            .map(ToOwned::to_owned)
            .unwrap_or(path.clone());

        let file = if let Some(stem) = parsed_name {
            // Ensure unique filenames by appending a counter
            let count = unique_files.entry(stem.clone()).or_insert(0);
            *count += 1;
            let stem = format!("{}_{}", stem, count);
            TestFile::from_file(path, relative_path, &stem)
        } else {
            bail!(
                r#"Invalid filename: {path:?}. Expected "<valid-nonempty-utf8>.rs"
Note that the tests/fail directory is only allowed to contain compile-fail test files."#
            )
        };
        test_files.push(file);
    }

    if test_files.is_empty() {
        message::no_tests();
        return Ok(());
    }

    filter(&mut test_files);

    if test_files.is_empty() {
        message::no_tests_enabled();
        return Ok(());
    }

    let project = Project::prepare()?;

    print_col!("\n\n");

    let tests_dir = project.dir.join("tests");
    std::fs::create_dir_all(&tests_dir)?;

    let mut active_test_files = HashSet::new();

    for file in &test_files {
        if file.error.is_some() {
            continue;
        }

        for test in &file.test_cases {
            let test_file_path = tests_dir.join(&test.filename);

            // Only write if content has changed
            let current_content = std::fs::read(&test_file_path).ok();
            if current_content.as_deref() != Some(test.source_code.as_bytes()) {
                std::fs::write(&test_file_path, &test.source_code)?;
            }

            active_test_files.insert(test.filename.clone());
        }
    }

    // Clean up test files that are no longer in the input set
    for entry in walkdir::WalkDir::new(&tests_dir).min_depth(1).max_depth(1) {
        let Ok(entry) = entry else { continue };
        if !entry.file_type().is_file() {
            continue;
        }
        if let Some(filename) = entry.file_name().to_str()
            && !active_test_files.contains(filename)
        {
            std::fs::remove_file(entry.path())?;
        }
    }

    let mut output = cargo::check_tests(&project)?;

    let mut total = 0;
    let mut failed = 0;
    for file in test_files {
        if file.error.is_some() {
            total += 1;
            failed += 1;
            message::begin_test("err_span_check file parse", &file.relative_path, 0);
            message::fail(file.error.unwrap());
            continue;
        }

        let mut new_file_content = String::new();
        for test in &file.test_cases {
            message::begin_test(
                &test.display_name,
                &file.relative_path,
                test.start_line_number,
            );
            total += 1;

            let local_path = PathBuf::from("tests").join(&test.filename);
            let full_path = project.dir.join(&local_path);
            let full_path = full_path.canonicalize().unwrap_or(full_path);

            let test_output = output.remove(&full_path).unwrap_or_default();

            if test_output.is_empty() {
                message::should_not_have_compiled();
                failed += 1;
                new_file_content.push_str(&test.expected);
                continue;
            }

            let normalize = normalize::Normalizer::new(&project, &local_path, &file.relative_path);

            let actual = test.annotate_with(&test_output, &normalize);
            if test.expected == actual {
                message::ok();
            } else if project.should_update {
                message::updated(&file.relative_path);
                failed += 1;
            } else {
                message::mismatch(&test.expected, &actual);
                failed += 1;
            }
            new_file_content.push_str(&actual);
        }

        if project.should_update && new_file_content != file.original_content {
            std::fs::write(&file.path, new_file_content)?;
        }
    }

    print_col!("\n\n");

    if failed > 0 {
        panic!("{failed} of {total} tests failed");
    }

    Ok(())
}

// Filter which test cases are run by err_span_check.
//
//     $ cargo test -- ui err_span_check=tuple_structs.rs
//
// The first argument after `--` must be the err_span_check test name i.e. the name of
// the function that has the #[test] attribute and calls err_span_check. That's to get
// Cargo to run the test at all. The next argument starting with `err_span_check=`
// provides a filename filter. Only test cases whose filename contains the
// filter string will be run.
fn filter(tests: &mut Vec<TestFile>) {
    let filters = std::env::args_os()
        .filter_map(|arg| {
            let s = arg.as_os_str().to_str()?;
            s.strip_prefix("err_span_check=").map(String::from)
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<String>>();

    if filters.is_empty() {
        return;
    }

    tests.retain(|t| {
        let path = t.path.to_string_lossy();
        filters.iter().any(|f| path.contains(f))
    });
}
