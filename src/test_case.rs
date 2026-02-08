use super::{normalize::Normalizer, *};

use std::{fmt::Write, iter::Peekable};

use cargo_metadata::diagnostic::Diagnostic;

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
    /// Line number in TestFile where this test case originates.
    pub start_line_number: usize,
    /// The header line of this test case.
    header_line: String,
    /// Flag if the test case had a BLOCK_SEPARATOR at the end or if it flows into the next test case / end of file.
    has_end_separator: bool,
    /// The expected output for this test case.
    pub expected: String,
    /// The source code of this test case, without any error annotations.
    pub source_code: String,
    /// The source code lines as a vector.
    source_code_lines: Vec<String>,
}

impl TestFile {
    pub fn from_error(path: PathBuf, relative_path: PathBuf, error: Error) -> Self {
        TestFile {
            path,
            relative_path,
            original_content: String::new(),
            test_cases: vec![],
            error: Some(error),
        }
    }

    pub fn from_file(path: PathBuf, relative_path: PathBuf, file_stem: &str) -> Self {
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

        TestFile {
            path,
            relative_path,
            original_content,
            test_cases,
            error: None,
        }
    }
}

/// Indicator used to mark a break. Can be: Start of a test case, ERRORS_HEADER, or BLOCK_SEPARATOR.
const META_INDICATOR: &str = "/////";
const ERRORS_HEADER: &str = "//////////////////// errors ////////////////////";
const BLOCK_SEPARATOR: &str =
    "////////////////////////////////////////////////////////////////////////////////";

/// Takes lines from the input iterator until it encounters a META_INDICATOR, without consuming the META_INDICATOR line.
fn take_content_block<'a, 'input: 'a, I: Iterator<Item = (usize, &'input str)>>(
    lines: &'a mut Peekable<I>,
) -> impl Iterator<Item = (usize, &'input str)> + 'a {
    let iter = lines.by_ref();
    std::iter::from_fn(move || iter.next_if(|(_, line)| !line.starts_with(META_INDICATOR)))
}

const INLINE_MARKER: &str = "//~";
const E: &str = ""; // print n repetitions of a character by printing an empty string with padding

impl TestCase {
    pub fn from_lines<'a, I>(
        file_stem: &str,
        (start_line_number, start_line): (usize, &'a str),
        lines: &mut Peekable<I>,
        test_case_index: usize,
    ) -> std::result::Result<Self, (usize, String)>
    where
        I: Iterator<Item = (usize, &'a str)>,
    {
        if !start_line.starts_with(META_INDICATOR) {
            // TODO: add support for setup code
            let msg = format!(
                r#"Invalid test case start line.
Test cases have a header line that starts with at least "{META_INDICATOR}".
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

        // parse source code
        for (_, line) in take_content_block(lines) {
            writeln!(expected, "{line}").unwrap();

            if !line.trim_start().starts_with(INLINE_MARKER) {
                writeln!(source_code, "{line}").unwrap();
                source_code_lines.push(line.to_string());
            }
        }

        if let Some((_, header)) = lines.next_if(|(_, l)| *l == ERRORS_HEADER) {
            writeln!(expected, "{header}").unwrap();

            for (_, line) in take_content_block(lines) {
                writeln!(expected, "{line}").unwrap();
            }
        }

        let has_end_separator;
        if let Some((_, separator)) = lines.next_if(|(_, l)| *l == BLOCK_SEPARATOR) {
            has_end_separator = true;

            writeln!(expected, "{separator}").unwrap();
        } else {
            has_end_separator = false;
        };

        // Normalize the number of trailing newlines
        expected.truncate(expected.trim_end().len());
        writeln!(expected).unwrap();

        // Generate semi-stable identifier based on file stem and test number
        let filename = format!("{}_{}.rs", file_stem, test_case_index);

        Ok(TestCase {
            filename,
            display_name,
            start_line_number,
            header_line: start_line.to_owned(),
            has_end_separator,
            expected,
            source_code,
            source_code_lines,
        })
    }

    pub(crate) fn annotate_with(&self, errors: &[Diagnostic], normalize: &Normalizer) -> String {
        let mut annotations = vec![vec![]; self.source_code_lines.len()];

        let mut remaining_errors = vec![];
        for error in errors {
            if let Some((line, annotation)) = self.to_annotation(error, normalize) {
                annotations[line].push(annotation);
            } else {
                let normalized =
                    normalize.diagnostics(error.rendered.as_deref().unwrap_or_default());
                remaining_errors.push(normalized);
            }
        }

        let mut actual = String::new();
        writeln!(actual, "{}", &self.header_line).unwrap();
        for (line, annotation) in self.source_code_lines.iter().zip(&mut annotations) {
            writeln!(actual, "{line}").unwrap();

            // By default, errors are emitted left to right:
            //
            // my_fn(some_wrong_arg, some_other_wrong_arg);
            // //~   ^^^^^^^^^^^^^^ this is text of the first error message
            // //~                   ^^^^^^^^^^^^^^^^^^^^ this is text of the second error message
            //
            // However, this looks like the carets of the second error are pointing to the text of the first error, not
            // the code. To avoid this confusion, we sort the annotations by descending starting byte offset:
            //
            // my_fn(some_wrong_arg, some_other_wrong_arg);
            // //~                   ^^^^^^^^^^^^^^^^^^^^ this is text of the second error message
            // //~   ^^^^^^^^^^^^^^ this is text of the first error message
            annotation.sort_by_key(|(byte_start, _)| std::cmp::Reverse(*byte_start));
            for (_, inline) in annotation {
                writeln!(actual, "{inline}").unwrap();
            }
        }

        if !remaining_errors.is_empty() {
            writeln!(actual, "{ERRORS_HEADER}").unwrap();
            writeln!(actual).unwrap();

            // Append remaining errors as comments
            for error in &remaining_errors {
                for line in error.lines() {
                    if line.trim().is_empty() {
                        writeln!(actual, "//").unwrap(); // no space after slashes
                    } else {
                        writeln!(actual, "// {line}").unwrap();
                    }
                }
                writeln!(actual).unwrap();
            }
        }

        if self.has_end_separator {
            writeln!(actual, "{BLOCK_SEPARATOR}").unwrap();
        }

        // Normalize the number of trailing newlines
        actual.truncate(actual.trim_end().len());
        writeln!(actual).unwrap();

        actual
    }

    /// Tries to convert a compiler diagnostic message into an inline annotation
    fn to_annotation(
        &self,
        msg: &Diagnostic,
        normalize: &Normalizer,
    ) -> Option<(usize, (u32, String))> {
        let primary = msg.spans.iter().find(|s| s.is_primary)?;

        if primary.line_start != primary.line_end {
            return None; // Can't annotate multi-line spans inline
        }

        let line = primary.line_start - 1; // zero-based line number

        let source_line = self.source_code_lines.get(line)?;

        // Prefix: "    //~"
        let mut prefix = source_line
            .chars()
            .take_while(|&b| b.is_whitespace())
            .collect::<String>();
        prefix += INLINE_MARKER;

        let mut chars = source_line.chars();

        let start = chars
            .by_ref()
            .take(primary.column_start - 1)
            .collect::<String>();
        let start_len = unicode_width::UnicodeWidthStr::width(start.as_str());
        let spaces = start_len.checked_sub(prefix.len())?;

        let underlined = chars
            .take(primary.column_end - primary.column_start)
            .collect::<String>();
        let underlined_len = unicode_width::UnicodeWidthStr::width(underlined.as_str());
        let carets = underlined_len.max(1); // empty spans (.start() or .end()) are indicated with at least one caret

        // First line prefix: "    //~    ^^^^^^^^ "
        let caret_line = format!("{prefix}{E: <spaces$}{E:^<carets$} ");

        // Following line prefix: "    //~             "
        write!(prefix, "{E: <spaces$}{E: <carets$} ").unwrap();

        let message = normalize.message(&msg.message);

        let mut out = String::new();
        if let Some(label) = &primary.label
            && label != &msg.message
        {
            let label = normalize.message(label);

            // Write:
            //     //~  ^^^^^^^^ error: message0
            //     //~                  message1
            //     //~           label: label0
            //     //~                  label1

            const MESSAGE_PREFIX: &str = "error: ";
            let message_line = format!("{caret_line}{MESSAGE_PREFIX}");
            let message_indent = format!("{prefix}{E: <0$}", MESSAGE_PREFIX.len());
            write_indented(&mut out, &message_line, &message_indent, &message);

            const LABEL_PREFIX: &str = "label: ";
            let label_line = format!("{prefix}{LABEL_PREFIX}");
            let label_indent = format!("{prefix}{E: <0$}", LABEL_PREFIX.len());
            write_indented(&mut out, &label_line, &label_indent, &label);
        } else {
            // Write:
            //     //~  ^^^^^^^^ message0
            //     //~           message1
            write_indented(&mut out, &caret_line, &prefix, &message);
        }

        if out.ends_with('\n') {
            out.pop(); // remove trailing newline
        }

        Some((line, (primary.byte_start, out)))
    }
}

/// Write a string with different prefixes for the first line and the following lines
fn write_indented(f: &mut String, first_line: &str, indentation: &str, text: &str) {
    let mut prefix = Some(first_line);
    for line in text.lines() {
        let prefix = prefix.take().unwrap_or(indentation);
        writeln!(f, "{prefix}{line}").unwrap();
    }
}
