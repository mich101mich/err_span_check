use cargo_metadata::{Message, diagnostic::DiagnosticLevel};

use crate::{normalize, normalize::Context, project::Project, *};

pub(crate) struct ParsedOutputs {
    pub stdout: String,
    pub stderrs: BTreeMap<PathBuf, Stderr>,
}

pub(crate) struct Stderr {
    pub success: bool,
    pub stderr: normalize::Variations,
}

impl Default for Stderr {
    fn default() -> Self {
        Stderr {
            success: true,
            stderr: normalize::Variations::default(),
        }
    }
}

pub(crate) fn parse_cargo_json(
    project: &Project,
    stdout: &[u8],
    path_map: &BTreeMap<PathBuf, (&str, &Path)>,
) -> ParsedOutputs {
    let mut map = BTreeMap::<PathBuf, Stderr>::new();
    let mut nonmessage_stdout = String::new();
    let mut seen = std::collections::HashSet::new();

    for message in Message::parse_stream(stdout) {
        // unwrap: only fails if read failed, but we have all data in memory
        let msg = match message.unwrap() {
            Message::CompilerMessage(msg) => msg,
            Message::TextLine(text) => {
                nonmessage_stdout.push_str(&text);
                nonmessage_stdout.push('\n');
                continue;
            }
            _ => continue, // Don't care about other messages
        };

        if msg.message.level == DiagnosticLevel::FailureNote {
            continue;
        }

        if seen.contains(&msg) {
            // Discard duplicate messages. This might no longer be necessary
            // after https://github.com/rust-lang/rust/issues/106571 is fixed.
            // Normally rustc would filter duplicates itself and I think this is
            // a short-lived bug.
            continue;
        }
        seen.insert(msg.clone());

        let src_path = msg.target.src_path;
        let src_path = src_path
            .canonicalize()
            .unwrap_or_else(|_| src_path.into_std_path_buf());
        let Some((name, test)) = path_map.get(&src_path) else {
            continue;
        };
        let entry = map.entry(src_path).or_default();
        if msg.message.level == DiagnosticLevel::Error {
            entry.success = false;
        }
        let context = Context {
            krate: name,
            source_dir: &project.source_dir,
            workspace: &project.workspace,
            input_file: test,
            target_dir: &project.target_dir,
            path_dependencies: &project.path_dependencies,
        };
        let normalized = normalize::diagnostics(&msg.message.rendered.unwrap_or_default(), context);
        entry.stderr.concat(&normalized);
    }

    ParsedOutputs {
        stdout: nonmessage_stdout,
        stderrs: map,
    }
}
