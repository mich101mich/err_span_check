use crate::{
    cargo::{self, Stderr, parse_cargo_json},
    normalize::Variations,
    project::Project,
    util::env::Update,
    *,
};

struct Report {
    failures: usize,
}

#[derive(Debug)]
pub(crate) struct Test {
    pub name: String,
    pub path: PathBuf,
}

pub(crate) fn run() -> Result<()> {
    let base_dir = cargo::manifest_dir()?;
    let fail_dir = base_dir.join("tests/fail");
    if !fail_dir.is_dir() {
        // We could say "no tests found" here, but the user explicitly called this function, so they want to run tests.
        // If the directory is missing, it probably means they set up their project incorrectly.
        return Err(Error::NoFailDir(base_dir));
    }

    let mut tests = vec![];

    for entry in walkdir::WalkDir::new(fail_dir).follow_links(true) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }
        let index = tests.len();
        let name = format!("err_span_check{:03}", index);
        tests.push(Test {
            name,
            path: path.to_owned(),
        });
    }

    if tests.is_empty() {
        message::no_tests();
        return Ok(());
    }

    filter(&mut tests);

    let mut project = Project::prepare(&tests)?;
    project.write()?;

    print_col!("\n\n");

    let len = tests.len();
    let mut report = Report { failures: 0 };

    if tests.is_empty() {
        message::no_tests_enabled();
        return Ok(());
    } else if project.keep_going {
        report = match run_all(&project, tests) {
            Ok(failures) => failures,
            Err(err) => {
                message::fail(err);
                Report { failures: len }
            }
        }
    } else {
        for test in tests {
            match run_test(&test.path, &project, &test.name) {
                Ok(()) => {}
                Err(err) => {
                    report.failures += 1;
                    message::fail(err);
                }
            }
        }
    }

    print_col!("\n\n");

    if report.failures > 0 {
        panic!("{} of {} tests failed", report.failures, len);
    }
    Ok(())
}

fn run_all(project: &Project, tests: Vec<Test>) -> Result<Report> {
    let mut report = Report { failures: 0 };
    let mut path_map = BTreeMap::new();
    for t in &tests {
        let src_path = project.source_dir.join(&t.path);
        let src_path = src_path.canonicalize().unwrap_or(src_path);
        path_map.insert(src_path, (t.name.as_str(), t.path.as_ref()));
    }

    let output = cargo::build_all_tests(project)?;
    let parsed = parse_cargo_json(project, &output.stdout, &path_map);
    let fallback = Stderr::default();

    for t in tests {
        message::begin_test(&t.path);

        let src_path = project.source_dir.join(&t.path);
        let src_path = src_path.canonicalize().unwrap_or(src_path);
        let this_test = parsed.stderrs.get(&src_path).unwrap_or(&fallback);
        match check_compile_fail(project, &t.name, this_test.success, "", &this_test.stderr) {
            Ok(()) => {}
            Err(error) => {
                report.failures += 1;
                message::fail(error);
            }
        }
    }

    Ok(report)
}

fn run_test(path: &Path, project: &Project, name: &str) -> Result<()> {
    message::begin_test(path);

    let mut path_map = BTreeMap::new();
    let src_path = project.source_dir.join(path);
    let src_path = src_path.canonicalize().unwrap_or(src_path);
    path_map.insert(src_path.clone(), (name, path));

    let output = cargo::build_test(project, name)?;
    let parsed = parse_cargo_json(project, &output.stdout, &path_map);
    let fallback = Stderr::default();
    let this_test = parsed.stderrs.get(&src_path).unwrap_or(&fallback);
    check_compile_fail(
        project,
        name,
        this_test.success,
        &parsed.stdout,
        &this_test.stderr,
    )
}

fn check_compile_fail(
    project: &Project,
    _name: &str,
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
fn filter(tests: &mut Vec<Test>) {
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
