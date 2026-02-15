use crate::*;

pub(crate) fn run() -> Result<()> {
    let start_time = std::time::Instant::now();

    let base_dir = cargo::manifest_dir()?;

    let mut test_files = fail_dir::parse_test_files(base_dir)?;

    if test_files.is_empty() {
        message::no_tests();
        return Ok(());
    }

    let original_count = test_files.len();
    filter(&mut test_files);
    let filtered = original_count - test_files.len();

    if test_files.is_empty() {
        message::no_tests_enabled();
        return Ok(());
    }

    let project = Project::prepare()?;

    print_col!("\n\n");

    let tests_dir = project.dir.join("tests");
    std::fs::create_dir_all(&tests_dir).context("failed to create tests directory")?;

    let mut active_test_files = vec![];

    for file in &test_files {
        if file.error.is_some() {
            continue;
        }

        for block in &file.blocks {
            let fail_dir::Block::TestCase { test_case, .. } = block else {
                continue;
            };
            let test_file_path = tests_dir.join(&test_case.filename);

            // Only write if content has changed
            let current_content = std::fs::read(&test_file_path).ok();
            if current_content.as_deref() != Some(test_case.source_code.as_bytes()) {
                std::fs::write(&test_file_path, &test_case.source_code)
                    .context("failed to write test file")?;
            }

            active_test_files.push(test_case.test_name.as_str());
        }
    }

    let mut output = cargo::check_tests(&project, &active_test_files)?;

    let mut total = 0;
    let mut failed = 0;
    for file in test_files {
        if let Some(error) = file.error {
            total += 1;
            failed += 1;
            message::begin_test("err_span_check file parse", &file.relative_path, 0);
            message::fail(error);
            continue;
        } else if let Err(error) = file.status {
            // We refuse to even run tests on files that have unstaged changes to make tests deterministic.
            // Otherwise, they would succeed after one run due to updating.
            total += 1;
            failed += 1;
            message::begin_test("git status is clean", &file.relative_path, 0);
            message::fail(error);
            continue;
        }

        let mut new_file_content = String::new();
        let mut last_was_test_case = false;
        for block in &file.blocks {
            let test = match block {
                fail_dir::Block::TestCase { test_case, .. } => test_case,
                fail_dir::Block::Code(range) => {
                    last_was_test_case = false;
                    for line in &file.setup_code[range.clone()] {
                        new_file_content.push_str(line);
                        new_file_content.push('\n');
                    }
                    continue;
                }
            };

            if last_was_test_case {
                new_file_content.push('\n');
            }

            last_was_test_case = true;

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
            file.write(new_file_content)?;
        }
    }

    print_col!("\n\n");

    if failed == 0 {
        let duration = start_time.elapsed();
        message::print_summary(total - failed, failed, filtered, duration);
    } else {
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
