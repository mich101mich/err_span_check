use super::*;

use std::fmt::Write;

use cargo_metadata::diagnostic::Diagnostic;

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
    /// A user-friendly name for this test case.
    pub display_name: String,
    /// The filename to use for this test case when writing it to disk.
    pub filename: String,
    pub start_line_number: usize,
    pub header_line: String,
    /// The expected output for this test case.
    pub expected: String,
    /// The source code of this test case, without any error annotations.
    pub source_code: String,
    /// The source code lines as a vector of (byte offset in file, line content).
    pub source_code_lines: Vec<(usize, String)>,
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

const HEADER_INDICATOR: &str = "/////";
const BLOCK_SEPARATOR: &str =
    "////////////////////////////////////////////////////////////////////////////////";
const INLINE_MARKER: &str = "//~";

impl TestCase {
    pub fn from_lines<'a, I>(
        file_stem: &str,
        (start_line_number, start_line): (usize, &'a str),
        lines: &mut std::iter::Peekable<I>,
        test_case_index: usize,
    ) -> std::result::Result<Self, (usize, String)>
    where
        I: Iterator<Item = (usize, &'a str)>,
    {
        if !start_line.starts_with(HEADER_INDICATOR) {
            let msg = format!(
                r#"Invalid test case start line.
Test cases have a header line that starts with at least "{HEADER_INDICATOR}".
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

        let mut expected = String::new();
        writeln!(expected, "{start_line}").unwrap();

        let mut source_code = String::new();
        let mut source_code_lines = vec![];
        let mut byte_offset = 0;
        for (_, line) in lines.by_ref() {
            writeln!(expected, "{line}").unwrap();

            if line.starts_with(HEADER_INDICATOR) {
                // start of remaining error block
                break;
            }

            if !line.trim_start().starts_with(INLINE_MARKER) {
                writeln!(source_code, "{line}").unwrap();
                source_code_lines.push((byte_offset, line.to_string()));
                byte_offset += line.len() + 1; // +1 for newline
            }
        }

        while let Some((_, line)) = lines.next_if(|(_, line)| !line.starts_with(HEADER_INDICATOR)) {
            writeln!(expected, "{line}").unwrap();
        }

        // Generate stable identifier based on file stem and test number
        let filename = format!("{}_{}.rs", file_stem, test_case_index);

        Ok(TestCase {
            filename,
            display_name,
            start_line_number,
            header_line: start_line.to_owned(),
            expected,
            source_code,
            source_code_lines,
        })
    }

    pub(crate) fn annotate(&self, errors: &[(Diagnostic, String)]) -> String {
        let mut annotations = vec![vec![]; self.source_code_lines.len()];

        let mut remaining_errors = String::new();
        for (e, normalized) in errors {
            if let Some((line, annotation)) = self.to_annotation(e) {
                annotations[line].push(annotation);
            } else {
                writeln!(remaining_errors, "{normalized}").unwrap();
            }
        }

        let mut actual = String::new();
        writeln!(actual, "{}", &self.header_line).unwrap();
        for ((_, line), annotation) in self.source_code_lines.iter().zip(&mut annotations) {
            writeln!(actual, "{line}").unwrap();

            // By default, errors are emitted left to right. However, that would look worse as an annotations:
            //
            // my_fn(some_wrong_arg, some_other_wrong_arg);
            // //~   ^^^^^^^^^^^^^^ this is text of the first error message
            // //~                   ^^^^^^^^^^^^^^^^^^^^ this is text of the second error message
            //
            // vs
            //
            // my_fn(some_wrong_arg, some_other_wrong_arg);
            // //~                   ^^^^^^^^^^^^^^^^^^^^ this is text of the second error message
            // //~   ^^^^^^^^^^^^^^ this is text of the first error message
            //
            // In the default case, the carets are only pointing at the error message, not the code.
            // => Sort by descending starting byte offset.
            annotation.sort_by_key(|(byte_start, _)| std::cmp::Reverse(*byte_start));
            for (_, inline) in annotation {
                writeln!(actual, "{inline}").unwrap();
            }
        }

        if !actual.ends_with("\n\n") {
            // make sure there's a blank line before the separator
            writeln!(actual).unwrap();
        }
        writeln!(actual, "{BLOCK_SEPARATOR}").unwrap();

        // Append remaining errors as comments
        for line in remaining_errors.lines() {
            writeln!(actual, "// {line}").unwrap();
        }

        actual
    }

    /// Tries to convert a compiler diagnostic message into an inline annotation
    fn to_annotation(&self, msg: &Diagnostic) -> Option<(usize, (u32, String))> {
        let primary = msg.spans.iter().find(|s| s.is_primary)?;

        if primary.line_start != primary.line_end {
            return None; // Can't annotate multi-line spans inline
        }

        let line = primary.line_start - 1; // zero-based line number

        let (byte_offset, source_line) = self.source_code_lines.get(line)?;

        let indentation = source_line
            .chars()
            .take_while(|&b| b.is_whitespace())
            .collect::<String>();

        let num_prefix_spaces = (primary.byte_start as usize)
            .checked_sub(byte_offset + indentation.len() + INLINE_MARKER.len())?;

        // empty spans (.start() or .end()) are indicated with at least one caret
        let num_carets = (primary.byte_end - primary.byte_start).max(1) as usize;

        let message = msg.message.replace('\n', "\\n");

        let mut inline_annotation = String::new();
        // Write "    //~    ^^^^^^^^ message"
        write!(
            inline_annotation,
            "{indentation}{INLINE_MARKER}{0: <1$}{0:^<2$} {message}",
            "", num_prefix_spaces, num_carets,
        )
        .unwrap();

        if let Some(label) = &primary.label {
            let label = label.replace('\n', " \\n ");
            write!(inline_annotation, ": {label}").unwrap();
        }

        Some((line, (primary.byte_start, inline_annotation)))
    }
}
