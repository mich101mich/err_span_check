use crate::*;

use std::{fs::File, process::Command};

use cargo_metadata::{
    Message,
    diagnostic::{Diagnostic, DiagnosticLevel},
};

fn raw_cargo() -> Command {
    match std::env::var_os("CARGO") {
        Some(cargo) => Command::new(cargo),
        None => Command::new("cargo"),
    }
}

fn cargo(project: &Project) -> Command {
    let mut cmd = raw_cargo();
    cmd.current_dir(&project.dir);
    cmd.env_remove("RUSTFLAGS");
    cmd.env(
        "CARGO_TARGET_DIR",
        project.target_dir.join("tests").join("err_span_check"),
    );
    cmd.env("CARGO_INCREMENTAL", "0");
    cmd.arg("--offline");

    let rustflags = rustflags::toml();
    cmd.arg(format!("--config=build.rustflags={rustflags}"));
    cmd.arg(format!(
        "--config=target.{}.rustflags={rustflags}",
        target_triple::TARGET
    ));

    cmd
}

pub(crate) fn manifest_dir() -> Result<PathBuf> {
    if let Some(manifest_dir) = std::env::var_os("CARGO_MANIFEST_DIR") {
        return Ok(PathBuf::from(manifest_dir));
    }
    for dir in std::env::current_dir()?.ancestors() {
        if dir.join("Cargo.toml").exists() {
            return Ok(dir.to_path_buf());
        }
    }
    bail!("failed to determine name of project dir")
}

pub(crate) fn build_dependencies(project: &mut Project) -> Result<()> {
    // Try copying or generating lockfile.
    match File::open(project.workspace.join("Cargo.lock")) {
        Ok(mut workspace_cargo_lock) => {
            if let Ok(mut new_cargo_lock) = File::create(project.dir.join("Cargo.lock")) {
                // Not fs::copy in order to avoid producing a read-only destination
                // file if the source file happens to be read-only.
                let _ = std::io::copy(&mut workspace_cargo_lock, &mut new_cargo_lock);
            }
        }
        Err(err) => {
            if err.kind() == std::io::ErrorKind::NotFound {
                let _ = cargo(project).arg("generate-lockfile").status();
            }
        }
    }

    let mut command = cargo(project);
    command
        .arg("check")
        .args(target())
        .arg("--bin")
        .arg(&project.name)
        .args(features(project));

    let status = command.status().context("failed to execute cargo")?;
    if !status.success() {
        bail!("cargo reported an error")
    }

    Ok(())
}

pub(crate) fn check_tests(project: &Project) -> Result<HashMap<PathBuf, Vec<Diagnostic>>> {
    cargo(project)
        .arg("check")
        .arg("--tests")
        .args(features(project))
        .args(target())
        .arg("--quiet")
        .arg("--color=never")
        .arg("--message-format=json")
        .arg("--keep-going")
        .output()
        .context("failed to execute cargo")
        .map(|out| parse_cargo_json(&out.stdout))
}

pub(crate) fn metadata() -> Result<cargo_metadata::Metadata> {
    cargo_metadata::MetadataCommand::new()
        .no_deps()
        .exec()
        .map_err(Into::into)
}

fn features(project: &Project) -> Vec<String> {
    match &project.features {
        Some(features) => vec![
            "--no-default-features".to_owned(),
            "--features".to_owned(),
            features.join(","),
        ],
        None => vec![],
    }
}

fn target() -> Vec<&'static str> {
    vec!["--target", target_triple::TARGET]
}

pub(crate) fn parse_cargo_json(stdout: &[u8]) -> HashMap<PathBuf, Vec<Diagnostic>> {
    let mut stderrs = HashMap::<PathBuf, Vec<Diagnostic>>::new();
    let mut seen = HashSet::new();

    for message in Message::parse_stream(stdout) {
        // unwrap: only fails if read failed, but we have all data in memory
        let msg = match message.unwrap() {
            Message::CompilerMessage(msg) => msg,
            Message::TextLine(text) => {
                println!("{text}");
                continue;
            }
            _ => continue, // Don't care about other messages
        };

        if msg.message.level != DiagnosticLevel::Error {
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

        let src_path = &msg.target.src_path;
        let src_path = src_path
            .canonicalize()
            .unwrap_or_else(|_| src_path.as_std_path().to_owned());

        stderrs.entry(src_path).or_default().push(msg.message);
    }

    stderrs
}
