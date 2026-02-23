#[cfg(test)]
mod tests;

use super::*;

pub(crate) struct Normalizer {
    /// Name of the crate being tested -> becomes `$CRATE`
    krate: String,
    /// Normalized path to the target directory -> becomes `$OUT_DIR`
    target_dir_pat: String,
    /// Normalized path to the input test file
    input_file_pat: String,
    /// Normalized path to the workspace directory -> becomes `$WORKSPACE`
    workspace_pat: String,
    /// Path to replace input_file_pat with in the output, since test files are split
    replaced_path: String,
    /// Dependencies from local paths
    path_dependencies: Vec<(String, String)>,
}

impl Normalizer {
    pub(crate) fn new(project: &Project, local_path: &Path, replaced_path: &Path) -> Self {
        let input_file_pat = path_to_pat(local_path);
        let target_dir_pat = dir_to_pat(&project.target_dir);
        let workspace_pat = dir_to_pat(&project.workspace);

        let replaced_path = replaced_path.to_string_lossy().replace('\\', "/");

        let path_dependencies = project
            .path_dependencies
            .iter()
            .map(|path_dep| {
                let name = format!("${}", path_dep.name.to_uppercase().replace('-', "_"));
                let path_dep_pat = dir_to_pat(&path_dep.normalized_path);
                (name, path_dep_pat)
            })
            .collect::<Vec<_>>();

        Normalizer {
            krate: project.name.to_owned(),
            target_dir_pat,
            input_file_pat,
            workspace_pat,
            replaced_path,
            path_dependencies,
        }
    }

    pub(crate) fn message(&self, output: &str) -> String {
        // replace the file in case a proc macro uses the file name in the message
        replace_case_insensitive(output, &self.input_file_pat, &self.replaced_path)
    }

    pub(crate) fn diagnostics(&self, original: &str) -> String {
        let mut normalized = String::new();

        let lines: Vec<&str> = original.lines().collect();
        let mut filter = Filter {
            all_lines: &lines,
            context: self,
            hide_numbers: 0,
            other_types: None,
        };
        for i in 0..lines.len() {
            if let Some(line) = filter.apply(i) {
                normalized += &line;
                if !normalized.ends_with("\n\n") {
                    normalized.push('\n');
                }
            }
        }

        normalized.truncate(normalized.trim_end().len());

        trim(normalized)
    }
}

pub(crate) fn trim<S: AsRef<[u8]>>(output: S) -> String {
    let bytes = output.as_ref();
    let mut normalized = String::from_utf8_lossy(bytes).into_owned();

    let len = normalized.trim_end().len();
    normalized.truncate(len);

    if !normalized.is_empty() {
        normalized.push('\n');
    }

    normalized
}

struct Filter<'a> {
    all_lines: &'a [&'a str],
    context: &'a Normalizer,
    hide_numbers: usize,
    other_types: Option<usize>,
}

impl<'a> Filter<'a> {
    fn apply(&mut self, index: usize) -> Option<String> {
        let mut line = self.all_lines[index].to_owned();

        line.truncate(line.trim_end().len());

        if self.hide_numbers > 0 {
            hide_leading_numbers(&mut line);
            self.hide_numbers -= 1;
        }

        let trim_start = line.trim_start();
        let indent = line.len() - trim_start.len();
        let prefix = if trim_start.starts_with("--> ") {
            Some("--> ")
        } else if trim_start.starts_with("::: ") {
            Some("::: ")
        } else {
            None
        };

        if let Some(prefix) = prefix {
            line = line.replace('\\', "/");
            let line_lower = line.to_ascii_lowercase();

            let prefix_offset = indent + prefix.len();
            let after_prefix = &line_lower[prefix_offset..];

            let Normalizer {
                target_dir_pat,
                input_file_pat,
                workspace_pat,
                path_dependencies,
                replaced_path,
                ..
            } = &self.context;
            let mut other_crate = false;

            if after_prefix.starts_with(target_dir_pat) {
                let mut offset = prefix_offset + target_dir_pat.len();
                let mut out_dir_crate_name = None;
                while let Some(slash) = line[offset..].find('/') {
                    let component = &line[offset..offset + slash];
                    if component == "out" {
                        if let Some(out_dir_crate_name) = out_dir_crate_name {
                            let replacement = format!("$OUT_DIR[{}]", out_dir_crate_name);
                            line.replace_range(prefix_offset..offset + 3, &replacement);
                            other_crate = true;
                            break;
                        }
                    } else if component.len() > 17
                        && component.rfind('-') == Some(component.len() - 17)
                        && is_ascii_lowercase_hex(&component[component.len() - 16..])
                    {
                        out_dir_crate_name = Some(&component[..component.len() - 17]);
                    } else {
                        out_dir_crate_name = None;
                    }
                    offset += slash + 1;
                }
            } else if after_prefix.starts_with(input_file_pat) {
                // Keep line numbers only within the input file (the
                // path passed to our `fn compile_fail`. All other
                // source files get line numbers erased below.

                let range = prefix_offset..prefix_offset + input_file_pat.len();
                line.replace_range(range, replaced_path);
                return Some(line);
            } else if let Some(i) = line_lower.find(workspace_pat) {
                line.replace_range(i..i + workspace_pat.len() - 1, "$WORKSPACE");
                other_crate = true;
            }

            if !other_crate {
                for (name, path_dep_pat) in path_dependencies {
                    if let Some(i) = line_lower.find(path_dep_pat) {
                        line.replace_range(i..i + path_dep_pat.len() - 1, name);
                        other_crate = true;
                        break;
                    }
                }
            }

            if !other_crate {
                if let Some(pos) = line.find("/rustlib/src/rust/src/") {
                    // --> /home/.rustup/toolchains/nightly/lib/rustlib/src/rust/src/libstd/net/ip.rs:83:1
                    // --> $RUST/src/libstd/net/ip.rs:83:1
                    line.replace_range(indent + 4..pos + 17, "$RUST");
                    other_crate = true;
                } else if let Some(pos) = line.find("/rustlib/src/rust/library/") {
                    // --> /home/.rustup/toolchains/nightly/lib/rustlib/src/rust/library/std/src/net/ip.rs:83:1
                    // --> $RUST/std/src/net/ip.rs:83:1
                    line.replace_range(indent + 4..pos + 25, "$RUST");
                    other_crate = true;
                } else if line[indent + 4..].starts_with("/rustc/")
                    && line
                        .get(indent + 11..indent + 51)
                        .is_some_and(is_ascii_lowercase_hex)
                    && line[indent + 51..].starts_with("/library/")
                {
                    // --> /rustc/c5c7d2b37780dac1092e75f12ab97dd56c30861e/library/std/src/net/ip.rs:83:1
                    // --> $RUST/std/src/net/ip.rs:83:1
                    line.replace_range(indent + 4..indent + 59, "$RUST");
                    other_crate = true;
                }
            }
            if !other_crate
                && let Some(pos) = line
                    .find("/registry/src/github.com-")
                    .or_else(|| line.find("/registry/src/index.crates.io-"))
            {
                let hash_start = pos + line[pos..].find('-').unwrap() + 1;
                let hash_end = hash_start + 16;
                if line
                    .get(hash_start..hash_end)
                    .is_some_and(is_ascii_lowercase_hex)
                    && line[hash_end..].starts_with('/')
                {
                    // --> /home/.cargo/registry/src/github.com-1ecc6299db9ec823/serde_json-1.0.64/src/de.rs:2584:8
                    // --> $CARGO/serde_json-1.0.64/src/de.rs:2584:8
                    line.replace_range(indent + 4..hash_end, "$CARGO");
                    other_crate = true;
                    let rest = &line[indent + 11..];
                    let end_of_version = rest.find('/');
                    if let Some(end_of_crate_name) = end_of_version
                        .and_then(|end| rest[..end].find('.'))
                        .and_then(|end| rest[..end].rfind('-'))
                    {
                        line.replace_range(
                            indent + end_of_crate_name + 12..indent + end_of_version.unwrap() + 11,
                            "$VERSION",
                        );
                    }
                }
            }
            if other_crate {
                // Blank out line numbers for this particular error since rustc
                // tends to reach into code from outside of the test case. The
                // test stderr shouldn't need to be updated every time we touch
                // those files.
                hide_trailing_numbers(&mut line);
                self.hide_numbers = 1;
                while let Some(next_line) = self.all_lines.get(index + self.hide_numbers) {
                    match next_line.trim_start().chars().next().unwrap_or_default() {
                        '0'..='9' | '|' | '.' => self.hide_numbers += 1,
                        _ => break,
                    }
                }
            }
            return Some(line);
        }

        if line == "To learn more, run the command again with --verbose." {
            return None;
        }

        if trim_start.starts_with("= note: this compiler was built on 2")
            && trim_start.ends_with("; consider upgrading it if it is out of date")
        {
            return None;
        }

        if line.starts_with("error: aborting due to ")
            || line.starts_with("error: could not compile `")
            || line.starts_with("error: Could not compile `")
            || line.starts_with("For more information about this error, try `rustc --explain")
            || line.starts_with("Some errors have detailed explanations:")
            || line.starts_with("For more information about an error, try `rustc --explain")
        {
            return None;
        }

        if line
            .trim_start()
            .starts_with("= note: required because it appears within the type")
        {
            line = line.replace('\\', "/");
        }

        let trim_start = line.trim_start();
        if let Some(right) = trim_start.strip_prefix("and ")
            && let Some(middle) = right.strip_suffix(" others")
            && middle.bytes().all(|b| b.is_ascii_digit())
        {
            line = line.replace(middle, "$N");
        }

        let trimmed_line = line.trim_start();
        let trimmed_line = trimmed_line
            .strip_prefix("= note: ")
            .unwrap_or(trimmed_line);
        if trimmed_line.starts_with("the full type name has been written to")
            || trimmed_line.starts_with("the full name for the type has been written to")
        {
            return None;
        }

        let trim_start = line.trim_start();
        if trim_start.starts_with("= help: the following types implement trait ")
            || trim_start.starts_with("= help: the following other types implement trait ")
        {
            self.other_types = Some(0);
        } else if let Some(count_other_types) = &mut self.other_types {
            if indent >= 12 && trim_start != "and $N others" {
                *count_other_types += 1;
                if *count_other_types == 9 {
                    if let Some(next) = self.all_lines.get(index + 1) {
                        let next_trim_start = next.trim_start();
                        let next_indent = next.len() - next_trim_start.len();
                        if indent == next_indent {
                            line.replace_range(indent - 2.., "and $N others");
                        }
                    }
                } else if *count_other_types > 9 {
                    return None;
                }
            } else {
                self.other_types = None;
            }
        }

        line = line.replace(&self.context.krate, "$CRATE");
        line = replace_case_insensitive(
            &line,
            &self.context.input_file_pat,
            &self.context.replaced_path,
        );
        line = replace_case_insensitive(&line, &self.context.workspace_pat, "$WORKSPACE/");

        Some(line)
    }
}

fn path_to_pat(path: &Path) -> String {
    path.to_string_lossy()
        .to_ascii_lowercase()
        .replace('\\', "/")
}

fn dir_to_pat(path: &Path) -> String {
    let mut pat = path_to_pat(path);
    if !pat.ends_with('/') {
        pat.push('/');
    }
    pat
}

fn is_ascii_lowercase_hex(s: &str) -> bool {
    s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

// "10 | T: Send,"  ->  "   | T: Send,"
fn hide_leading_numbers(line: &mut String) {
    let n = line
        .bytes()
        .take_while(|b: &u8| *b == b' ' || b.is_ascii_digit())
        .count();
    for i in 0..n {
        line.replace_range(i..i + 1, " ");
    }
}

// "main.rs:22:29"  ->  "main.rs"
fn hide_trailing_numbers(line: &mut String) {
    for _ in 0..2 {
        let digits = line.bytes().rev().take_while(u8::is_ascii_digit).count();
        if digits == 0 || !line[..line.len() - digits].ends_with(':') {
            return;
        }
        line.truncate(line.len() - digits - 1);
    }
}

fn replace_case_insensitive(line: &str, pattern: &str, replacement: &str) -> String {
    let line_lower = line.to_ascii_lowercase().replace('\\', "/");
    let pattern_lower = pattern.to_ascii_lowercase().replace('\\', "/");
    let mut replaced = String::with_capacity(line.len());

    let line_lower = line_lower.as_str();
    let mut split = line_lower.split(&pattern_lower);
    let mut pos = 0;
    let mut insert_replacement = false;
    while let Some(keep) = split.next() {
        if insert_replacement {
            replaced.push_str(replacement);
            pos += pattern.len();
        }
        let mut keep = &line[pos..pos + keep.len()];
        if insert_replacement {
            let end_of_maybe_path = keep.find(&[' ', ':'][..]).unwrap_or(keep.len());
            replaced.push_str(&keep[..end_of_maybe_path].replace('\\', "/"));
            pos += end_of_maybe_path;
            keep = &keep[end_of_maybe_path..];
        }
        replaced.push_str(keep);
        pos += keep.len();
        insert_replacement = true;
        if replaced.ends_with(|ch: char| ch.is_ascii_alphanumeric())
            && let Some(ch) = line[pos..].chars().next()
        {
            replaced.push(ch);
            pos += ch.len_utf8();
            split = line_lower[pos..].split(&pattern_lower);
            insert_replacement = false;
        }
    }

    replaced
}
