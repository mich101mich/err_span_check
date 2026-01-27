use crate::{
    Runner, Test,
    cargo::{self, Stderr, parse_cargo_json},
    expand::ExpandedTest,
    normalize::Variations,
    project::Project,
    util::{env::Update, flock::Lock},
    *,
};

struct Report {
    failures: usize,
    created_wip: usize,
}

impl Runner {
    pub(crate) fn run(&mut self) {
        let mut tests = expand::expand_globs(&self.tests);
        filter(&mut tests);

        let (project, _lock) = (|| {
            let mut project = self.prepare(&tests)?;
            let lock = Lock::acquire(project.dir.join(".lock"))?;
            self.write(&mut project)?;
            Ok((project, lock))
        })()
        .unwrap_or_else(|err| {
            message::prepare_fail(err);
            panic!("tests failed");
        });

        print_col!("\n\n");

        let len = tests.len();
        let mut report = Report {
            failures: 0,
            created_wip: 0,
        };

        if tests.is_empty() {
            message::no_tests_enabled();
        } else if project.keep_going {
            report = match self.run_all(&project, tests) {
                Ok(failures) => failures,
                Err(err) => {
                    message::test_fail(err);
                    Report {
                        failures: len,
                        created_wip: 0,
                    }
                }
            }
        } else {
            for test in tests {
                match test.run(&project) {
                    Ok(Outcome::Passed) => {}
                    Ok(Outcome::CreatedWip) => report.created_wip += 1,
                    Err(err) => {
                        report.failures += 1;
                        message::test_fail(err);
                    }
                }
            }
        }

        print_col!("\n\n");

        if report.failures > 0 {
            panic!("{} of {} tests failed", report.failures, len);
        }
        if report.created_wip > 0 {
            panic!(
                "successfully created new stderr files for {} test cases",
                report.created_wip,
            );
        }
    }

    fn run_all(&self, project: &Project, tests: Vec<ExpandedTest>) -> Result<Report> {
        let mut report = Report {
            failures: 0,
            created_wip: 0,
        };

        let mut path_map = BTreeMap::new();
        for t in &tests {
            let src_path = project.source_dir.join(&t.test.path);
            let src_path = src_path.canonicalize().unwrap_or(src_path);
            path_map.insert(src_path, (t.name.as_str(), &t.test));
        }

        let output = cargo::build_all_tests(project)?;
        let parsed = parse_cargo_json(project, &output.stdout, &path_map);
        let fallback = Stderr::default();

        for mut t in tests {
            message::begin_test(&t.test);

            if t.error.is_none() {
                t.error = check_exists(&t.test.path).err();
            }

            if t.error.is_none() {
                let src_path = project.source_dir.join(&t.test.path);
                let src_path = src_path.canonicalize().unwrap_or(src_path);
                let this_test = parsed.stderrs.get(&src_path).unwrap_or(&fallback);
                match t.test.check_compile_fail(
                    project,
                    &t.name,
                    this_test.success,
                    "",
                    &this_test.stderr,
                ) {
                    Ok(Outcome::Passed) => {}
                    Ok(Outcome::CreatedWip) => report.created_wip += 1,
                    Err(error) => t.error = Some(error),
                }
            }

            if let Some(err) = t.error {
                report.failures += 1;
                message::test_fail(err);
            }
        }

        Ok(report)
    }
}

enum Outcome {
    Passed,
    CreatedWip,
}

impl Test {
    fn run(&self, project: &Project, name: &str) -> Result<Outcome> {
        message::begin_test(self);
        check_exists(&self.path)?;

        let mut path_map = BTreeMap::new();
        let src_path = project.source_dir.join(&self.path);
        let src_path = src_path.canonicalize().unwrap_or(src_path);
        path_map.insert(src_path.clone(), (name, self));

        let output = cargo::build_test(project, name)?;
        let parsed = parse_cargo_json(project, &output.stdout, &path_map);
        let fallback = Stderr::default();
        let this_test = parsed.stderrs.get(&src_path).unwrap_or(&fallback);
        self.check_compile_fail(
            project,
            name,
            this_test.success,
            &parsed.stdout,
            &this_test.stderr,
        )
    }

    fn check_compile_fail(
        &self,
        project: &Project,
        _name: &str,
        success: bool,
        build_stdout: &str,
        variations: &Variations,
    ) -> Result<Outcome> {
        let preferred = variations.preferred();

        if success {
            message::should_not_have_compiled();
            message::fail_output(message::Fail, build_stdout);
            message::warnings(preferred);
            return Err(Error::ShouldNotHaveCompiled);
        }

        let stderr_path = self.path.with_extension("stderr");

        if !stderr_path.exists() {
            let outcome = match project.update {
                Update::None => {
                    let wip_dir = Path::new("wip");
                    std::fs::create_dir_all(wip_dir)?;
                    let gitignore_path = wip_dir.join(".gitignore");
                    std::fs::write(gitignore_path, "*\n")?;
                    let stderr_name = stderr_path
                        .file_name()
                        .unwrap_or_else(|| std::ffi::OsStr::new("test.stderr"));
                    let wip_path = wip_dir.join(stderr_name);
                    message::write_stderr_wip(&wip_path, &stderr_path, preferred);
                    std::fs::write(wip_path, preferred).map_err(Error::WriteStderr)?;
                    Outcome::CreatedWip
                }
                Update::Overwrite => {
                    message::overwrite_stderr(&stderr_path, preferred);
                    std::fs::write(stderr_path, preferred).map_err(Error::WriteStderr)?;
                    Outcome::Passed
                }
            };
            message::fail_output(message::Warn, build_stdout);
            return Ok(outcome);
        }

        let expected = std::fs::read_to_string(&stderr_path)
            .map_err(Error::ReadStderr)?
            .replace("\r\n", "\n");

        if variations.any(|stderr| expected == stderr) {
            message::ok();
            return Ok(Outcome::Passed);
        }

        match project.update {
            Update::None => {
                message::mismatch(&expected, preferred);
                Err(Error::Mismatch)
            }
            Update::Overwrite => {
                message::overwrite_stderr(&stderr_path, preferred);
                std::fs::write(stderr_path, preferred).map_err(Error::WriteStderr)?;
                Ok(Outcome::Passed)
            }
        }
    }
}

fn check_exists(path: &Path) -> Result<()> {
    if path.exists() {
        return Ok(());
    }
    match std::fs::File::open(path) {
        Ok(_) => Ok(()),
        Err(err) => Err(Error::Open(path.to_owned(), err)),
    }
}

impl ExpandedTest {
    fn run(self, project: &Project) -> Result<Outcome> {
        match self.error {
            None => self.test.run(project, &self.name),
            Some(error) => {
                message::begin_test(&self.test);
                Err(error)
            }
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
fn filter(tests: &mut Vec<ExpandedTest>) {
    let filters = std::env::args_os()
        .flat_map(std::ffi::OsString::into_string)
        .filter_map(|mut arg| {
            const PREFIX: &str = "err_span_check=";
            if arg.starts_with(PREFIX) && arg != PREFIX {
                Some(arg.split_off(PREFIX.len()))
            } else {
                None
            }
        })
        .collect::<Vec<String>>();

    if filters.is_empty() {
        return;
    }

    tests.retain(|t| {
        filters
            .iter()
            .any(|f| t.test.path.to_string_lossy().contains(f))
    });
}
