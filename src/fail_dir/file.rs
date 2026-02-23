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
            error: Some(error),
        }
    }

    pub fn from_file(
        path: PathBuf,
        relative_path: PathBuf,
        file_stem: &str,
        original_content: String,
    ) -> Self {
        let (setup_code, tests) = match Self::parse_test_cases(file_stem, &original_content) {
            Ok(value) => value,
            Err((line_number, e)) => {
                let error = anyhow::anyhow!(
                    "Failed to parse test case from {}:{line_number}: {e}",
                    path.display(),
                );
                return Self::from_error(path, relative_path, error);
            }
        };

        TestFile {
            path,
            relative_path,
            original_content,
            setup_code,
            tests,
            git_status: Ok(()),
            error: None,
        }
    }

    pub fn copy_from(
        existing: TestFile,
        path: PathBuf,
        relative_path: PathBuf,
        file_stem: &str,
        original_content: String,
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
                writeln!(new_content, "{}", &self.setup_code[setup_code_index]).unwrap();
                setup_code_index += 1;
            }

            match run(test) {
                RunResult::UseExpected => new_content.push_str(&test.expected),
                RunResult::Update { actual } => new_content.push_str(&actual),
            }
        }

        while setup_code_index < self.setup_code.len() {
            writeln!(new_content, "{}", &self.setup_code[setup_code_index]).unwrap();
            setup_code_index += 1;
        }

        new_content
    }

    pub(crate) fn write(&self, new_file_content: String) -> Result<(), Error> {
        let parent_dir = self.path.parent().path_context(
            &self.relative_path,
            "Failed to get parent directory for test file: <path>",
        )?;
        std::fs::create_dir_all(parent_dir).path_context(
            &self.relative_path,
            "Failed to create directories for test file: <path>",
        )?;

        std::fs::write(&self.path, new_file_content).path_context(
            &self.relative_path,
            "Failed to write updated test file: <path>",
        )?;
        Ok(())
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

        let existing =
            TestFile::from_file(PathBuf::new(), PathBuf::new(), "file_a", file_a.to_string());
        let copied = TestFile::copy_from(
            existing,
            PathBuf::new(),
            PathBuf::new(),
            "file_b",
            file_b.to_string(),
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
}
