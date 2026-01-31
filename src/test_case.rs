use super::*;

#[derive(Debug)]
pub(crate) struct TestFile {
    pub path: PathBuf,
    pub original_content: String,
    pub test_cases: Vec<TestCase>,
    /// If an error occurred while processing this test file, it is stored here.
    /// This allows us to continue processing other test files.
    pub error: Option<Error>,
}

#[derive(Debug)]
pub(crate) struct TestCase {
    /// A globally unique identifier for this test case.
    pub filename: String,
    /// A user-friendly name for this test case.
    pub display_name: String,
    pub source_code: String,
    pub expected_errors: Vec<ExpectedErrors>,
}

#[derive(Debug)]
pub(crate) enum ExpectedErrors {
    Inline {
        line: usize,
        span: (usize, usize),
        message: String,
    },
    External(String),
}

impl TestFile {
    pub fn from_error(path: PathBuf, error: Error) -> Self {
        TestFile {
            path,
            original_content: String::new(),
            test_cases: vec![],
            error: Some(error),
        }
    }

    pub fn from_file(path: PathBuf, file_stem: &str) -> Self {
        let original_content = match std::fs::read_to_string(&path) {
            Ok(original_content) => original_content,
            Err(e) => return TestFile::from_error(path, e.into()),
        };

        let mut lines = original_content
            .split_inclusive('\n')
            .enumerate()
            .map(|(i, line)| (i + 1, line)) // line numbers start at 1
            .peekable();

        let mut test_cases = vec![];
        while let Some(start) = lines.next() {
            match TestCase::from_lines(file_stem, start, &mut lines) {
                Ok(test_case) => test_cases.push(test_case),
                Err((line_number, e)) => {
                    let error = Error::TestCaseParse(path.clone(), line_number, e);
                    return Self::from_error(path, error);
                }
            }
        }

        TestFile {
            path,
            original_content,
            test_cases,
            error: None,
        }
    }
}

impl TestCase {
    pub fn from_lines<'a, I>(
        file_stem: &str,
        (start_line_number, start_line): (usize, &'a str),
        lines: &mut std::iter::Peekable<I>,
    ) -> std::result::Result<Self, (usize, String)>
    where
        I: Iterator<Item = (usize, &'a str)>,
    {
        if !start_line.starts_with("/////") {
            let msg = format!(
                r#"Invalid test case start line.
Test cases have a header line that starts with at least 5 '/' characters.
Got: {start_line}"#
            );
            return Err((start_line_number, msg));
        }

        let display_name = start_line.trim_matches(|c: char| c == '/' || c.is_whitespace());
        let display_name = if !display_name.is_empty() {
            display_name.to_owned()
        } else {
            format!("Test at line {start_line_number}")
        };

        let mut source_code = String::new();
        let mut expected_errors = vec![];
        for (line_number, line) in lines.by_ref() {
            if line.starts_with("//////////") {
                break;
            }
            source_code.push_str(line);

            if line.trim_start().starts_with("//~") {
                let Some(span_start) = line.bytes().position(|b| b == b'^') else {
                    let msg = format!(
                        "Expected inline error annotation to contain '^' to indicate error span.\nGot: {line}"
                    );
                    return Err((line_number, msg));
                };
                let span_len = line[span_start..]
                    .bytes()
                    .take_while(|&b| b == b'^')
                    .count();
                let span_end = span_start + span_len;
                let message = line[span_end..].trim().to_owned();

                expected_errors.push(ExpectedErrors::Inline {
                    line: line_number - start_line_number + 1, // 1-based line number within test case
                    span: (span_start, span_end),
                    message,
                });
            }
        }

        match lines.peek() {
            None => {} // No external error message, end of file
            Some((_, line)) if line.starts_with("/////") => {} // No external error message, next test case
            Some(_) => {
                let mut error_message = String::new();
                while let Some((_, line)) = lines.next_if(|(_, l)| !l.starts_with("/////")) {
                    error_message.push_str(line);
                }
                expected_errors.push(ExpectedErrors::External(error_message));
            }
        }

        // Generate stable identifier based on file stem and line number
        let filename = format!("{}_line_{}.rs", file_stem, start_line_number);

        Ok(TestCase {
            filename,
            display_name,
            source_code,
            expected_errors,
        })
    }
}
