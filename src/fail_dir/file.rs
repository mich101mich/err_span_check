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
    /// The test cases or blocks of setup code contained in this test file.
    pub blocks: Vec<Block>,
    /// The git status of this file. Ok(()) means clean, errors are self-explanatory.
    pub status: Result<()>,
    /// If an error occurred while processing this test file, it is stored here.
    /// This allows us to continue processing other test files.
    pub error: Option<Error>,
}

#[derive(Debug)]
pub(crate) enum Block {
    Code(std::ops::Range<usize>), // range into self.setup_code
    TestCase {
        test_case: TestCase,
        /// the byte position of this test case in self.setup_code
        pos_in_setup_code: usize,
    },
}

impl TestFile {
    pub fn from_error(path: PathBuf, relative_path: PathBuf, error: Error) -> Self {
        TestFile {
            path,
            relative_path,
            original_content: String::new(),
            setup_code: vec![],
            blocks: vec![],
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
        let (setup_code, blocks) = match Self::parse_test_cases(file_stem, &original_content) {
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
            setup_code,
            blocks,
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
        let mut blocks = existing.blocks;
        if !original_content.is_empty()
            && let Ok((_, own_blocks)) = Self::parse_test_cases(file_stem, &original_content)
        {
            let own_cases = own_blocks
                .iter()
                .filter_map(|c| match c {
                    Block::TestCase { test_case, .. } => Some(test_case),
                    _ => None,
                })
                .map(|c| (&c.display_name, c))
                .collect::<HashMap<_, _>>();

            for block in &mut blocks {
                let Block::TestCase { test_case, .. } = block else {
                    continue;
                };
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
            setup_code: existing.setup_code,
            blocks,
            status: Ok(()), // We create this file, so it is clean by definition
            error: existing.error,
        }
    }

    fn parse_test_cases(
        file_stem: &str,
        original_content: &str,
    ) -> std::result::Result<(Vec<String>, Vec<Block>), (usize, String)> {
        let mut lines = original_content
            .lines()
            .enumerate()
            .map(|(i, line)| (i + 1, line)) // line numbers start at 1
            .peekable();

        let mut blocks = vec![];
        let mut setup_code = vec![];
        let mut test_case_index = 0;
        loop {
            let block_start = setup_code.len();
            for (_, line) in TestCase::take_until_meta(&mut lines) {
                setup_code.push(line.to_string());
            }
            let block_end = setup_code.len();
            if block_start != block_end {
                blocks.push(Block::Code(block_start..block_end));
            }

            if lines.peek().is_none() {
                break;
            }

            let test_case =
                TestCase::from_lines(file_stem, &mut lines, test_case_index, &setup_code)?;
            blocks.push(Block::TestCase {
                test_case,
                pos_in_setup_code: setup_code.len(),
            });
            test_case_index += 1;
        }

        if test_case_index == 0 {
            return Err((1, "no test cases found".to_string()));
        }

        for block in blocks.iter_mut() {
            if let Block::TestCase {
                test_case,
                pos_in_setup_code,
            } = block
                && let Some(setup_code_block) = setup_code.get(*pos_in_setup_code..)
            {
                test_case.add_suffix(setup_code_block);
            }
        }

        Ok((setup_code, blocks))
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
