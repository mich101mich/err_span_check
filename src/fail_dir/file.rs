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
        repo: &git::GitRepo,
    ) -> Self {
        let original_content = match std::fs::read_to_string(&path) {
            Ok(original_content) => original_content,
            Err(e) => return Self::from_error(path, relative_path, e.into()),
        };

        let mut lines = original_content
            .lines()
            .enumerate()
            .map(|(i, line)| (i + 1, line)) // line numbers start at 1
            .peekable();

        let mut test_cases = vec![];
        while let Some(start) = lines.next() {
            let test_case_index = test_cases.len();
            match TestCase::from_lines(file_stem, start, &mut lines, test_case_index) {
                Ok(test_case) => test_cases.push(test_case),
                Err((line_number, e)) => {
                    let error = anyhow::anyhow!(
                        "Failed to parse test case from {}:{line_number}: {e}",
                        path.display(),
                    );
                    return Self::from_error(path, relative_path, error);
                }
            }
        }

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
}
