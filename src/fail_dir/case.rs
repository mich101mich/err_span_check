use super::*;

use std::{fmt::Write, iter::Peekable};

use cargo_metadata::diagnostic::Diagnostic;

#[derive(Debug)]
pub(crate) struct TestCase {
    /// A user-friendly name for this test case.
    pub display_name: String,
    /// How cargo will refer to this test case. The filename without the .rs.
    pub test_name: String,
    /// Line number in TestFile where this test case originates.
    pub start_line_number: usize,
    /// The header line of this test case.
    header_line: String,
    /// Line containing a BLOCK_SEPARATOR from the end of the test case, if any.
    end_separator: Option<String>,
    /// The expected output for this test case.
    pub expected: String,
    /// The source code of this test case, without any error annotations.
    pub source_code: String,
    /// The source code lines as a vector.
    source_code_lines: Vec<String>,
    /// Number of lines of setup code before this test case
    pub setup_code_prefix_length: usize,
}

/// Indicator used to mark a break. Can be: Start of a test case or BLOCK_SEPARATOR.
const META_INDICATOR: &str = "/////";

const ERRORS_HEADER: &str = "//~~~~~~~~~~~~~~~~~~~~ errors ~~~~~~~~~~~~~~~~~~~~//";
const ERROR_HEADER_INDICATOR: &str = "//~~~";

const INLINE_MARKER: &str = "//~";

const E: &str = "";

impl TestCase {
    pub fn filename(&self) -> String {
        format!("{}.rs", self.test_name)
    }

    /// Takes lines from the input iterator until it encounters a META_INDICATOR, without consuming the META_INDICATOR line.
    pub(crate) fn take_until_meta<'a, 'input: 'a, I: Iterator<Item = (usize, &'input str)>>(
        lines: &'a mut Peekable<I>,
    ) -> impl Iterator<Item = (usize, &'input str)> + 'a {
        let iter = lines.by_ref();
        std::iter::from_fn(move || {
            iter.next_if(|(_, line)| {
                let trimmed = line.trim_start();
                !trimmed.starts_with(META_INDICATOR) && !trimmed.starts_with(ERROR_HEADER_INDICATOR)
            })
        })
    }

    fn parse_header(line: &str) -> Option<&str> {
        // trim the slashes and spaces. This has to be multi-step trimming to preserve intentional slashes.
        // turn "    ///// my test ending with slash/ /////" into "my test ending with slash/"
        let line = line.trim(); // remove leading and trailing whitespace
        if !line.starts_with(META_INDICATOR) {
            return None;
        }
        let text = line
            .trim_matches('/') // remove the ///// blocks
            .trim(); // remove whitespace between the slashes and the header text
        (!text.is_empty()).then_some(text)
    }

    pub(crate) fn from_lines<'a, I>(
        test_name: String,
        lines: &mut Peekable<I>,
        setup_code_prefix: &[String],
    ) -> std::result::Result<Self, (usize, String)>
    where
        I: Iterator<Item = (usize, &'a str)>,
    {
        let (start_line_number, start_line) = lines.next().expect(
            "logic error in err_span_check: TestCase::from_lines called with empty lines iterator",
        );

        let Some(display_name) = Self::parse_header(start_line) else {
            let msg = format!(
                r#"Failed to parse test case header: expected a line like 
{META_INDICATOR} <name> {META_INDICATOR}
but got
{start_line}"#
            );
            return Err((start_line_number, msg));
        };

        let mut expected = String::new();
        writeln!(expected, "{start_line}").unwrap();

        let mut source_code = String::new();
        let mut source_code_lines = vec![];

        let setup_code_prefix_length = setup_code_prefix.len();
        for line in setup_code_prefix {
            writeln!(source_code, "{}", line).unwrap();
        }

        // parse source code
        for (_, line) in Self::take_until_meta(lines) {
            writeln!(expected, "{line}").unwrap();

            if !line.trim_start().starts_with(INLINE_MARKER) {
                writeln!(source_code, "{line}").unwrap();
                source_code_lines.push(line.to_string());
            }
        }

        if let Some((_, header)) =
            lines.next_if(|(_, l)| l.trim().starts_with(ERROR_HEADER_INDICATOR))
        {
            writeln!(expected, "{header}").unwrap();

            for (_, line) in Self::take_until_meta(lines) {
                writeln!(expected, "{line}").unwrap();
            }
        }

        let mut end_separator = None;
        // We are either at the end of the file, at a header, or at a block separator
        // => a line that is not a header must be a separator.
        if let Some((_, separator)) = lines.next_if(|(_, l)| Self::parse_header(l).is_none()) {
            end_separator = Some(separator.to_string());
            writeln!(expected, "{separator}").unwrap();
        };

        // Normalize the number of trailing newlines
        expected.truncate(expected.trim_end().len());
        writeln!(expected).unwrap();

        Ok(TestCase {
            test_name,
            display_name: display_name.to_owned(),
            start_line_number,
            header_line: start_line.to_owned(),
            end_separator,
            expected,
            source_code,
            source_code_lines,
            setup_code_prefix_length,
        })
    }

    pub(crate) fn add_suffix(&mut self, suffix: &[String]) {
        for line in &suffix[self.setup_code_prefix_length..] {
            writeln!(self.source_code, "{}", line).unwrap();
        }
    }

    pub(crate) fn annotate_with(
        &self,
        errors: &[Diagnostic],
        normalize: &normalize::Normalizer,
    ) -> String {
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

        let indentation = self
            .header_line
            .chars()
            .take_while(|c| c.is_whitespace())
            .collect::<String>();

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
            // the code. To avoid this confusion, we sort the annotations by descending starting column:
            //
            // my_fn(some_wrong_arg, some_other_wrong_arg);
            // //~                   ^^^^^^^^^^^^^^^^^^^^ this is text of the second error message
            // //~   ^^^^^^^^^^^^^^ this is text of the first error message
            annotation.sort_by_key(|(column_start, _)| std::cmp::Reverse(*column_start));
            for (_, inline) in annotation {
                writeln!(actual, "{inline}").unwrap();
            }
        }

        if !remaining_errors.is_empty() {
            writeln!(actual, "{indentation}{ERRORS_HEADER}").unwrap();
            writeln!(actual).unwrap();

            // Append remaining errors as comments
            for error in &remaining_errors {
                for line in error.lines() {
                    if line.trim().is_empty() {
                        writeln!(actual, "{indentation}//").unwrap(); // no space after slashes
                    } else {
                        writeln!(actual, "{indentation}// {line}").unwrap();
                    }
                }
                writeln!(actual).unwrap();
            }
        }

        if let Some(separator) = &self.end_separator {
            writeln!(actual, "{separator}").unwrap();
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
        normalize: &normalize::Normalizer,
    ) -> Option<(usize, (usize, String))> {
        let primary = msg.spans.iter().find(|s| s.is_primary)?;

        if primary.line_start != primary.line_end {
            return None; // Can't annotate multi-line spans inline
        }

        let line = primary
            .line_start
            .checked_sub(1 + self.setup_code_prefix_length)?; // zero-based line number

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

        Some((line, (primary.column_start, out)))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_take_until_meta() {
        let with_header_meta = "a\nb\n///// header\nc\n//~~~~~header 2\nd";
        let mut iter = with_header_meta.lines().enumerate().peekable();

        // read until first header
        let taken: Vec<_> = TestCase::take_until_meta(&mut iter).collect();
        assert_eq!(taken, vec![(0, "a"), (1, "b")]);

        // header is not consumed
        assert_eq!(iter.peek(), Some(&(2, "///// header")));

        // immediately at meta: no lines taken
        let taken: Vec<_> = TestCase::take_until_meta(&mut iter).collect();
        assert_eq!(taken, vec![]);

        // consume header
        assert_eq!(iter.next().unwrap(), (2, "///// header"));

        // read until next header
        let taken: Vec<_> = TestCase::take_until_meta(&mut iter).collect();
        assert_eq!(taken, vec![(3, "c")]);

        // consume next header
        assert_eq!(iter.next().unwrap(), (4, "//~~~~~header 2"));

        // read until end of file
        let taken: Vec<_> = TestCase::take_until_meta(&mut iter).collect();
        assert_eq!(taken, vec![(5, "d")]);
    }

    #[test]
    fn test_parse_header() {
        // Validating examples in Readme:

        // // valid starts:
        // ///// a name /////
        let header0 = "///// a name /////";
        // //////////////////////////////any number of slashes, nothing after it
        let header1 = "//////////////////////////////any number of slashes, nothing after it";
        //     ///// indentation /////
        let header2 = "    ///// indentation /////";
        // ///// /name/with/slash/ /////
        let header3 = "///// /name/with/slash/ /////";
        // // The above cases would be named "a name", "any number of slashes, nothing after it", "indentation", and "/name/with/slash/".
        assert_eq!(TestCase::parse_header(header0), Some("a name"));
        assert_eq!(
            TestCase::parse_header(header1),
            Some("any number of slashes, nothing after it")
        );
        assert_eq!(TestCase::parse_header(header2), Some("indentation"));
        assert_eq!(TestCase::parse_header(header3), Some("/name/with/slash/"));

        // // invalid: Not enough slashes
        // /// a name ///
        let invalid0 = "/// a name ///";
        // // invalid: No name
        // ////////////////////////////////////////
        let invalid1 = "////////////////////////////////////////";

        assert_eq!(TestCase::parse_header(invalid0), None);
        assert_eq!(TestCase::parse_header(invalid1), None);
    }

    #[test]
    fn test_annotate_with() {
        use cargo_metadata::diagnostic::Diagnostic;
        use serde_json::json;

        let source = r#"
fn foo() {
    let x = 1;
    let y = "🙂" ➕ 1️⃣;
    let z = 3;
}
"#;
        let mut lines = source.lines().map(String::from).collect::<Vec<_>>();
        if lines.first().map(|l| l.is_empty()).unwrap_or(false) {
            lines.remove(0);
        }

        // Construct TestCase manually
        let test_case = TestCase {
            display_name: "test".into(),
            test_name: "test".into(),
            start_line_number: 1,
            header_line: "///// test /////".into(),
            end_separator: None,
            expected: String::new(),
            source_code: source.trim_start().into(),
            source_code_lines: lines,
            setup_code_prefix_length: 0,
        };

        fn make_diagnostic(
            message: &str,
            (line_start, col_start): (usize, usize),
            (line_end, col_end): (usize, usize),
            rendered: &str,
        ) -> Diagnostic {
            // use serde_json to construct Diagnostic instances since the struct fields are non-exhaustive
            let span_json = json!({
                "file_name": "test.rs",
                "byte_start": 0, // unused
                "byte_end": 0, // unused
                "line_start": line_start,
                "line_end": line_end,
                "column_start": col_start,
                "column_end": col_end,
                "is_primary": true,
                "text": [],
                "label": null,
                "suggested_replacement": null,
                "suggestion_applicability": null,
                "expansion": null
            });

            let diagnostic_json = json!({
                "message": message,
                "code": null,
                "level": "error",
                "spans": [span_json],
                "children": [],
                "rendered": rendered
            });

            serde_json::from_value(diagnostic_json).expect("Failed to create diagnostic from JSON")
        }

        // 1. Multiple diagnostics on one line with different spans
        // Line 2: let x = 1;
        // Spans: "x" (col 9-10), "1" (col 13-14)
        let d1 = make_diagnostic("error 1", (2, 9), (2, 10), "unused");
        let d2 = make_diagnostic("error 2", (2, 13), (2, 14), "unused");

        // 2. Diagnostic in a line after a multibyte unicode char
        // Line 3: let y = "🙂" ➕ 1️⃣;
        // Span: "+" (col 17-18, note that the emoji is 2 columns wide)
        let d3 = make_diagnostic("after unicode", (3, 17), (3, 18), "unused");

        // 3. Diagnostic spanning multiple lines (should be ignored by annotate_with and fall back to rendered)
        // Line 4-5
        let d4 = make_diagnostic(
            "multiline",
            (4, 5),
            (5, 5),
            "error: multiline\n  --> test.rs:4:5\n",
        );

        let errors = vec![d1, d2, d3, d4];

        // Dummy project for normalizer
        let project = Project {
            dir: PathBuf::from("."),
            owned_dir: PathBuf::from("."),
            target_dir: PathBuf::from("target"),
            name: "test_crate".into(),
            should_update: false,
            features: None,
            workspace: PathBuf::from("."),
            path_dependencies: vec![],
        };
        let normalize =
            normalize::Normalizer::new(&project, Path::new("test.rs"), Path::new("test.rs"));

        let actual = test_case.annotate_with(&errors, &normalize);

        let expected = r#"///// test /////
fn foo() {
    let x = 1;
    //~     ^ error 2
    //~ ^ error 1
    let y = "🙂" ➕ 1️⃣;
    //~          ^^ after unicode
    let z = 3;
}
//~~~~~~~~~~~~~~~~~~~~ errors ~~~~~~~~~~~~~~~~~~~~//

// error: multiline
//   --> test.rs:4:5
"#;

        assert_eq!(actual.trim(), expected.trim());
    }

    #[test]
    fn test_write_indented() {
        //     //~  ^^^^^^^^ error: message0
        //     //~                  message1
        let first_line = "    //~  ^^^^^^^^ error: ";
        let indentation = "    //~                  ";
        let text = "message0\nmessage1";
        let mut out = String::new();
        write_indented(&mut out, first_line, indentation, text);
        assert_eq!(
            out,
            "    //~  ^^^^^^^^ error: message0\n    //~                  message1\n"
        );

        let first_line = "a ";
        let indentation = "<-=->";
        let text = "line1\nline2\nline3\n";
        let mut out = String::new();
        write_indented(&mut out, first_line, indentation, text);
        assert_eq!(out, "a line1\n<-=->line2\n<-=->line3\n");

        let text = "single line message";
        let mut out = String::new();
        write_indented(&mut out, first_line, indentation, text);
        assert_eq!(out, "a single line message\n");
    }
}
