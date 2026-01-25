use crate::{Test, normalize, normalize::Context, project::Project, *};

#[derive(Deserialize)]
pub(crate) struct CargoMessage {
    #[allow(dead_code)]
    reason: Reason,
    target: RustcTarget,
    message: RustcMessage,
}

#[derive(Deserialize)]
pub(crate) enum Reason {
    #[serde(rename = "compiler-message")]
    CompilerMessage,
}

#[derive(Deserialize)]
pub(crate) struct RustcTarget {
    src_path: PathBuf,
}

#[derive(Deserialize)]
pub(crate) struct RustcMessage {
    rendered: String,
    level: String,
}

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
    path_map: &BTreeMap<PathBuf, (&str, &Test)>,
) -> ParsedOutputs {
    let mut map = BTreeMap::new();
    let mut nonmessage_stdout = String::new();
    let mut remaining = &*String::from_utf8_lossy(stdout);
    let mut seen = std::collections::HashSet::new();
    while !remaining.is_empty() {
        let Some(begin) = remaining.find("{\"reason\":") else {
            break;
        };
        let (nonmessage, rest) = remaining.split_at(begin);
        nonmessage_stdout.push_str(nonmessage);
        let len = match rest.find('\n') {
            Some(end) => end + 1,
            None => rest.len(),
        };
        let (message, rest) = rest.split_at(len);
        remaining = rest;
        if !seen.insert(message) {
            // Discard duplicate messages. This might no longer be necessary
            // after https://github.com/rust-lang/rust/issues/106571 is fixed.
            // Normally rustc would filter duplicates itself and I think this is
            // a short-lived bug.
            continue;
        }
        if let Ok(de) = serde_json::from_str::<CargoMessage>(message)
            && de.message.level != "failure-note"
        {
            let src_path = &de.target.src_path;
            let src_path = src_path.canonicalize().unwrap_or(src_path.clone());
            let Some((name, test)) = path_map.get(&src_path) else {
                continue;
            };
            let entry = map.entry(src_path).or_insert_with(Stderr::default);
            if de.message.level == "error" {
                entry.success = false;
            }
            let normalized = normalize::diagnostics(
                &de.message.rendered,
                Context {
                    krate: name,
                    source_dir: &project.source_dir,
                    workspace: &project.workspace,
                    input_file: &test.path,
                    target_dir: &project.target_dir,
                    path_dependencies: &project.path_dependencies,
                },
            );
            entry.stderr.concat(&normalized);
        }
    }
    nonmessage_stdout.push_str(remaining);
    ParsedOutputs {
        stdout: nonmessage_stdout,
        stderrs: map,
    }
}
