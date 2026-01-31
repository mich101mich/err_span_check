use std::collections::{HashMap, HashSet};

use crate::{
    cargo::{self, Stderr, parse_cargo_json},
    normalize::Variations,
    project::Project,
    util::env::Update,
    *,
};

pub(crate) fn run() -> Result<()> {
    let base_dir = cargo::manifest_dir()?;
    let fail_dir = base_dir.join("tests/fail");
    if !fail_dir.is_dir() {
        // We could say "no tests found" here, but the user explicitly called this function, so they want to run tests.
        // If the directory is missing, it probably means they set up their project incorrectly.
        return Err(Error::NoFailDir(base_dir));
    }

    let mut test_files = vec![];

    let file_iter = walkdir::WalkDir::new(fail_dir)
        .min_depth(1)
        .follow_links(true)
        .sort_by_file_name();

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
            .filter(|(stem, ext)| !stem.is_empty() && ext == &"rs")
            .map(|(stem, _)| stem.to_owned());

        let file = if let Some(stem) = parsed_name {
            // Ensure unique filenames by appending a counter
            let count = unique_files.entry(stem.clone()).or_insert(0);
            *count += 1;
            let stem = format!("{}_{}", stem, count);
            TestFile::from_file(path, &stem)
        } else {
            let error = Error::InvalidFilename(path.clone());
            TestFile::from_error(path, error)
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

    let mut failed = 0;
    let mut total = 0;
    let name_map = BTreeMap::new();
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

    let output = cargo::check_tests(&project)?;
    let parsed = parse_cargo_json(&project, &output.stdout, &name_map);
    let fallback = Stderr::default();

    for t in test_files {
        message::begin_test(&t.path);

        let src_path = project.source_dir.join(&t.path);
        let src_path = src_path.canonicalize().unwrap_or(src_path);
        let this_test = parsed.stderrs.get(&src_path).unwrap_or(&fallback);

        match check_compile_fail(&project, this_test.success, "", &this_test.stderr) {
            Ok(()) => {}
            Err(error) => {
                failed += 1;
                message::fail(error);
            }
        }
        total += 1;
    }

    print_col!("\n\n");

    if failed > 0 {
        panic!("{failed} of {total} tests failed");
    }

    Ok(())
}

fn check_compile_fail(
    project: &Project,
    success: bool,
    build_stdout: &str,
    variations: &Variations,
) -> Result<()> {
    let preferred = variations.preferred();

    if success {
        message::should_not_have_compiled();
        message::fail_output(build_stdout);
        message::warnings(preferred);
        return Err(Error::ShouldNotHaveCompiled);
    }

    let expected = String::new(); // TODO:

    if variations.any(|stderr| expected == stderr) {
        message::ok();
        return Ok(());
    }

    match project.update {
        Update::None => {
            message::mismatch(&expected, preferred);
            Err(Error::Mismatch)
        }
        Update::Overwrite => {
            // TODO:
            Ok(())
        }
    }
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
