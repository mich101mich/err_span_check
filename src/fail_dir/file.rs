use super::*;

#[derive(Debug)]
pub(crate) struct TestFile {
    /// The absolute path to this test file.
    pub path: PathBuf,
    /// The relative path to this test file from the base directory.
    pub relative_path: PathBuf,
    /// The original content of this test file.
    pub original_content: String,
    /// The code that is not part of any test case.
    pub setup_code: Vec<String>,
    /// The test cases contained in this test file.
    pub tests: Vec<TestCase>,
    /// The git status of this file. Ok(()) means clean, errors are self-explanatory.
    pub git_status: Result<()>,
    /// Indicates if this test file is inside a stable/nightly directory.
    pub toolchain: Toolchain,
    /// If an error occurred while processing this test file, it is stored here.
    /// This allows us to continue processing other test files.
    pub error: Option<Error>,
}

pub(crate) enum RunResult {
    UseExpected,
    Update { actual: String },
}

impl TestFile {
    pub fn from_error(path: PathBuf, relative_path: PathBuf, error: Error) -> Self {
        TestFile {
            path,
            relative_path,
            original_content: String::new(),
            setup_code: vec![],
            tests: vec![],
            git_status: Ok(()),
            toolchain: Toolchain::Both, // value doesn't matter here
            error: Some(error),
        }
    }

    pub fn from_raw_file(file: RawFile, stem: &str) -> Self {
        let (setup_code, tests) = match Self::parse_test_cases(stem, &file.content) {
            Ok(value) => value,
            Err((line_number, e)) => {
                let error = anyhow::anyhow!(
                    "Failed to parse test case from {}:{line_number}: {e}",
                    file.path.display(),
                );
                return Self::from_error(file.path, file.relative_path, error);
            }
        };

        TestFile {
            path: file.path,
            relative_path: file.relative_path,
            original_content: file.content,
            setup_code,
            tests,
            git_status: file.git_status,
            toolchain: file.toolchain,
            error: None,
        }
    }

    pub fn copy_from(
        existing: TestFile,
        path: PathBuf,
        relative_path: PathBuf,
        file_stem: &str,
        original_content: String,
        toolchain: Toolchain,
    ) -> Self {
        let mut tests = existing.tests;
        if !original_content.is_empty()
            && let Ok((_, own_tests)) = Self::parse_test_cases(file_stem, &original_content)
        {
            let own_tests = own_tests
                .iter()
                .map(|c| (&c.display_name, c))
                .collect::<HashMap<_, _>>();

            for test_case in &mut tests {
                let Some(own_case) = own_tests.get(&test_case.display_name) else {
                    continue;
                };
                if test_case.source_code == own_case.source_code {
                    test_case.start_line_number = own_case.start_line_number;
                    test_case.expected = own_case.expected.clone();
                }
            }
        }

        TestFile {
            path,
            relative_path,
            original_content,
            setup_code: existing.setup_code,
            tests,
            git_status: Ok(()), // We create this file, so it is clean by definition
            toolchain,
            error: existing.error,
        }
    }

    pub fn has_error(&self) -> bool {
        self.error.is_some() || self.git_status.is_err()
    }

    fn parse_test_cases(
        file_stem: &str,
        original_content: &str,
    ) -> std::result::Result<(Vec<String>, Vec<TestCase>), (usize, String)> {
        let mut lines = original_content
            .lines()
            .enumerate()
            .map(|(i, line)| (i + 1, line)) // line numbers start at 1
            .peekable();

        let mut tests = vec![];
        let mut setup_code = vec![];
        let mut test_case_index = 0;
        loop {
            for (_, line) in TestCase::take_until_meta(&mut lines) {
                setup_code.push(line.to_string());
            }

            if lines.peek().is_none() {
                break;
            }

            let test_name = format!("{}_{}", file_stem, test_case_index);
            test_case_index += 1;

            let test_case = TestCase::from_lines(test_name, &mut lines, &setup_code)?;
            tests.push(test_case);
        }

        if test_case_index == 0 {
            return Err((1, "no test cases found".to_string()));
        }

        for test_case in tests.iter_mut() {
            test_case.add_suffix(&setup_code);
        }

        Ok((setup_code, tests))
    }

    pub(crate) fn process_tests(&self, mut run: impl FnMut(&TestCase) -> RunResult) -> String {
        use std::fmt::Write;
        let mut new_content = String::new();
        let mut setup_code_index = 0;
        for (i, test) in self.tests.iter().enumerate() {
            if i > 0 && setup_code_index == test.setup_code_prefix_length {
                // directly adjacent test cases => insert blank line
                writeln!(new_content).unwrap();
            }
            while setup_code_index < test.setup_code_prefix_length {
                writeln!(new_content, "{}", self.setup_code[setup_code_index]).unwrap();
                setup_code_index += 1;
            }

            match run(test) {
                RunResult::UseExpected => new_content.push_str(&test.expected),
                RunResult::Update { actual } => new_content.push_str(&actual),
            }
        }

        while setup_code_index < self.setup_code.len() {
            writeln!(new_content, "{}", self.setup_code[setup_code_index]).unwrap();
            setup_code_index += 1;
        }

        new_content
    }

    pub(crate) fn write(&self, new_file_content: String) -> Result<()> {
        if let Some(parent_dir) = self.path.parent() {
            fs_err::create_dir_all(parent_dir)
                .context("Failed to create directories for test file")?;
        }
        // It is pretty much guaranteed that the path will have a parent, so the `else` branch here should be an error,
        // but that seems a bit extreme. fs_err::write will probably have more info in its error.

        fs_err::write(&self.path, new_file_content).context("Failed to write updated test file")?;
        Ok(())
    }

    pub(crate) fn split(&self, new_file_content: &str) -> Result<Option<Error>> {
        let is_nightly = rustc_version::version_meta()
            .is_ok_and(|meta| meta.channel == rustc_version::Channel::Nightly);
        self.split_for_channel(new_file_content, is_nightly)
    }

    fn split_for_channel(&self, new_file_content: &str, is_nightly: bool) -> Result<Option<Error>> {
        let file_name = self.path.file_name().unwrap(); // unwrap: these were checked during dir walk
        let dir = self.path.parent().unwrap();
        let stable_file = dir.join("stable").join(file_name);
        let nightly_file = dir.join("nightly").join(file_name);

        match (stable_file.is_file(), nightly_file.is_file()) {
            (true, true) => return Ok(Some(anyhow::anyhow!("split files already exist"))),
            (true, false) => return Ok(Some(anyhow::anyhow!("stable file already exists"))),
            (false, true) => return Ok(Some(anyhow::anyhow!("nightly file already exists"))),
            (false, false) => {}
        }

        fs_err::create_dir_all(stable_file.parent().unwrap())
            .context("Failed to create directories for splitting stable test file")?;
        fs_err::create_dir_all(nightly_file.parent().unwrap())
            .context("Failed to create directories for splitting nightly test file")?;

        if is_nightly {
            // The user previously ran stable, which produced (or at least verified) self.original_content.
            // Now we ran nightly tests, which produced the (different) new_file_content.
            // => Move original file to stable, and write new_file_content to a nightly file.
            fs_err::rename(&self.path, &stable_file)
                .context("Failed to move original file to split stable test file")?;
            fs_err::write(&nightly_file, new_file_content)
                .context("Failed to write to split nightly test file")?;
        } else {
            // vice versa from above: Move original file to nightly, and write new_file_content to stable.
            fs_err::rename(&self.path, &nightly_file)
                .context("Failed to move original file to split nightly test file")?;
            fs_err::write(&stable_file, new_file_content)
                .context("Failed to write to split stable test file")?;
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_test_cases() {
        let content = r#"
setup line 1
setup line 2
///// test case 1
expected output 1
//~ error line 1
    //~ error line 2
 //////////////////////////////
    setup line 3
  ////////////////// test case 2 /////////////////////
expected output 2"#;

        let (setup_code, tests) = TestFile::parse_test_cases("test_file", content).unwrap();

        assert_eq!(
            setup_code,
            vec!["", "setup line 1", "setup line 2", "    setup line 3"]
        );

        assert_eq!(tests.len(), 2);
        assert_eq!(tests[0].display_name, "test case 1");
        assert_eq!(tests[0].test_name, "test_file_0");
        assert_eq!(
            tests[0].expected,
            "///// test case 1\nexpected output 1\n//~ error line 1\n    //~ error line 2\n //////////////////////////////\n"
        );
        assert_eq!(
            tests[0].source_code,
            "\nsetup line 1\nsetup line 2\nexpected output 1\n    setup line 3\n"
        );
        assert_eq!(tests[0].setup_code_prefix_length, 3);

        assert_eq!(tests[1].display_name, "test case 2");
        assert_eq!(tests[1].test_name, "test_file_1");
        assert_eq!(
            tests[1].expected,
            "  ////////////////// test case 2 /////////////////////\nexpected output 2\n"
        );
        assert_eq!(
            tests[1].source_code,
            "\nsetup line 1\nsetup line 2\n    setup line 3\nexpected output 2\n"
        );
        assert_eq!(tests[1].setup_code_prefix_length, 4);
    }

    #[test]
    fn test_copy_from() {
        let file_a = r#"
///// identical
expected output 1
//~ error in a.identical
///// changed
expected output 2
//~ error in a.changed
///// only in a
expected output 3
//~ error in a.only_in_a"#;

        let file_b = r#"
///// identical
expected output 1
//~ error in b.identical
///// changed
changed expected output 2
//~ error in b.changed
///// only in b
expected output 4
//~ error in b.only_in_b"#;

        let existing = TestFile::from_raw_file(
            RawFile {
                path: PathBuf::new(),
                relative_path: PathBuf::new(),
                content: file_a.to_string(),
                git_status: Ok(()),
                toolchain: Toolchain::Stable,
            },
            "file_a",
        );
        let copied = TestFile::copy_from(
            existing,
            PathBuf::new(),
            PathBuf::new(),
            "file_b",
            file_b.to_string(),
            Toolchain::Nightly,
        );

        assert_eq!(copied.tests.len(), 3);

        // identical source code: use errors from b.
        assert_eq!(copied.tests[0].display_name, "identical");
        assert_eq!(
            copied.tests[0].expected,
            "///// identical\nexpected output 1\n//~ error in b.identical\n"
        );

        // changed: Used fully from a
        assert_eq!(copied.tests[1].display_name, "changed");
        assert_eq!(
            copied.tests[1].expected,
            "///// changed\nexpected output 2\n//~ error in a.changed\n"
        );

        // only in a: Used fully from a
        assert_eq!(copied.tests[2].display_name, "only in a");
        assert_eq!(
            copied.tests[2].expected,
            "///// only in a\nexpected output 3\n//~ error in a.only_in_a\n"
        );

        // only in b: Not present in copied
    }

    struct TempDir {
        dir: PathBuf,
    }

    impl TempDir {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "err_span_check_file_test_{}_{}",
                std::process::id(),
                name
            ));
            let _ = std::fs::remove_dir_all(&dir);
            fs_err::create_dir_all(&dir).unwrap();
            Self { dir }
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn make_test_file_in(dir: &Path, filename: &str, content: &str) -> TestFile {
        let path = dir.join(filename);
        fs_err::write(&path, content).unwrap();
        TestFile::from_raw_file(
            RawFile {
                path,
                relative_path: PathBuf::from(filename),
                content: content.to_string(),
                git_status: Ok(()),
                toolchain: Toolchain::Both,
            },
            filename.strip_suffix(".rs").unwrap_or(filename),
        )
    }

    #[test]
    fn test_write_and_split() {
        let temp = TempDir::new("write_and_split");

        // Test write
        let file = make_test_file_in(&temp.dir, "write_test.rs", "///// test\nold\n");
        file.write("///// test\nnew\n".to_string()).unwrap();
        assert_eq!(
            fs_err::read_to_string(&file.path).unwrap(),
            "///// test\nnew\n"
        );

        // Test split on nightly
        let nightly_dir = temp.dir.join("on_nightly");
        fs_err::create_dir_all(&nightly_dir).unwrap();
        let file = make_test_file_in(&nightly_dir, "test.rs", "///// test\nstable_content\n");
        assert!(
            file.split_for_channel("///// test\nnightly_content\n", true)
                .unwrap()
                .is_none()
        );
        assert!(!file.path.exists());
        assert_eq!(
            fs_err::read_to_string(nightly_dir.join("stable/test.rs")).unwrap(),
            "///// test\nstable_content\n"
        );
        assert_eq!(
            fs_err::read_to_string(nightly_dir.join("nightly/test.rs")).unwrap(),
            "///// test\nnightly_content\n"
        );

        // Both exist now -> should report "split files already exist"
        let file = make_test_file_in(&nightly_dir, "test.rs", "///// test\ncontent\n");
        assert_eq!(
            file.split("///// test\nnew\n")
                .unwrap()
                .unwrap()
                .to_string(),
            "split files already exist"
        );

        // Only stable exists -> should report "stable file already exists"
        fs_err::remove_file(nightly_dir.join("nightly/test.rs")).unwrap();
        assert_eq!(
            file.split_for_channel("///// test\nnew\n", true)
                .unwrap()
                .unwrap()
                .to_string(),
            "stable file already exists"
        );

        // Test split on stable
        let stable_dir = temp.dir.join("on_stable");
        fs_err::create_dir_all(&stable_dir).unwrap();
        let file = make_test_file_in(&stable_dir, "test.rs", "///// test\nnightly_content\n");
        assert!(
            file.split_for_channel("///// test\nstable_content\n", false)
                .unwrap()
                .is_none()
        );
        assert!(!file.path.exists());
        assert_eq!(
            fs_err::read_to_string(stable_dir.join("stable/test.rs")).unwrap(),
            "///// test\nstable_content\n"
        );
        assert_eq!(
            fs_err::read_to_string(stable_dir.join("nightly/test.rs")).unwrap(),
            "///// test\nnightly_content\n"
        );

        // Only nightly exists -> should report "nightly file already exists"
        fs_err::remove_file(stable_dir.join("stable/test.rs")).unwrap();
        let file = make_test_file_in(&stable_dir, "test.rs", "///// test\ncontent\n");
        assert_eq!(
            file.split_for_channel("///// test\nnew\n", false)
                .unwrap()
                .unwrap()
                .to_string(),
            "nightly file already exists"
        );
    }
}
