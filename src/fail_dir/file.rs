use super::*;

#[derive(Debug)]
pub(crate) struct TestFile {
    /// The absolute path to this test file.
    pub path: PathBuf,
    /// The relative path to this test file from the base directory.
    pub relative_path: PathBuf,
    /// The original content of this test file.
    pub original_content: String,
    /// The test cases contained in this test file.
    pub test_cases: Vec<TestCase>,
    /// The git status of this file. Ok(()) means clean, errors are self-explanatory.
    pub status: Result<()>,
    /// If an error occurred while processing this test file, it is stored here.
    /// This allows us to continue processing other test files.
    pub error: Option<Error>,
}

impl TestFile {
    pub fn from_error(path: PathBuf, relative_path: PathBuf, error: Error) -> Self {
        TestFile {
            path,
            relative_path,
            original_content: String::new(),
            test_cases: vec![],
            status: Ok(()),
            error: Some(error),
        }
    }

    pub fn from_file(
        path: PathBuf,
        relative_path: PathBuf,
        file_stem: &str,
        original_content: String,
        repo: &git::GitRepo,
    ) -> Self {
        let test_cases = match Self::parse_test_cases(file_stem, &original_content) {
            Ok(value) => value,
            Err((line_number, e)) => {
                let error = anyhow::anyhow!(
                    "Failed to parse test case from {}:{line_number}: {e}",
                    path.display(),
                );
                return Self::from_error(path, relative_path, error);
            }
        };

        let status = repo.is_clean(&path);

        TestFile {
            path,
            relative_path,
            original_content,
            test_cases,
            status,
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
        let mut test_cases = existing.test_cases;
        if !original_content.is_empty()
            && let Ok(own_cases) = Self::parse_test_cases(file_stem, &original_content)
        {
            let own_cases = own_cases
                .iter()
                .map(|c| (&c.display_name, c))
                .collect::<HashMap<_, _>>();

            for test_case in &mut test_cases {
                let Some(own_case) = own_cases.get(&test_case.display_name) else {
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
            status: Ok(()), // We create this file, so it is clean by definition
            test_cases,
            error: existing.error,
        }
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

    fn parse_test_cases(
        file_stem: &str,
        original_content: &str,
    ) -> std::result::Result<Vec<TestCase>, (usize, String)> {
        let mut lines = original_content
            .lines()
            .enumerate()
            .map(|(i, line)| (i + 1, line)) // line numbers start at 1
            .peekable();
        let mut test_cases = vec![];
        while let Some(start) = lines.next() {
            let test_case_index = test_cases.len();
            let test_case = TestCase::from_lines(file_stem, start, &mut lines, test_case_index)?;
            test_cases.push(test_case);
        }
        Ok(test_cases)
    }
}
