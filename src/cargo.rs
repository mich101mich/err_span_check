use crate::{run::Project, *};

use std::{
    fs::File,
    process::{Command, Output, Stdio},
};

#[derive(Deserialize)]
pub(crate) struct Metadata {
    pub target_directory: PathBuf,
    pub workspace_root: PathBuf,
    pub packages: Vec<PackageMetadata>,
}

#[derive(Deserialize)]
pub(crate) struct PackageMetadata {
    pub name: String,
    pub targets: Vec<BuildTarget>,
    pub manifest_path: PathBuf,
}

#[derive(Deserialize)]
pub(crate) struct BuildTarget {
    pub crate_types: Vec<String>,
}

fn raw_cargo() -> Command {
    match std::env::var_os("CARGO") {
        Some(cargo) => Command::new(cargo),
        None => Command::new("cargo"),
    }
}

fn cargo(project: &Project) -> Command {
    let mut cmd = raw_cargo();
    cmd.current_dir(&project.dir);
    cmd.envs(cargo_target_dir(project));
    cmd.env_remove("RUSTFLAGS");
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

fn cargo_target_dir(project: &Project) -> impl Iterator<Item = (&'static str, PathBuf)> {
    std::iter::once((
        "CARGO_TARGET_DIR",
        project.target_dir.join("tests").join("err_span_check"),
    ))
}

pub(crate) fn manifest_dir() -> Result<PathBuf> {
    if let Some(manifest_dir) = std::env::var_os("CARGO_MANIFEST_DIR") {
        return Ok(PathBuf::from(manifest_dir));
    }
    let mut dir = std::env::current_dir()?;
    loop {
        if dir.join("Cargo.toml").exists() {
            return Ok(dir);
        }
        dir = dir.parent().ok_or(Error::ProjectDir)?.to_path_buf();
    }
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

    let status = command.status().map_err(Error::Cargo)?;
    if !status.success() {
        return Err(Error::CargoFail);
    }

    // Check if this Cargo contains https://github.com/rust-lang/cargo/pull/10383
    project.keep_going = command
        .arg("--keep-going")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());

    Ok(())
}

pub(crate) fn build_test(project: &Project, name: &str) -> Result<Output> {
    let _ = cargo(project)
        .arg("clean")
        .arg("--package")
        .arg(&project.name)
        .arg("--color=never")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    cargo(project)
        .arg("check")
        .args(target())
        .arg("--bin")
        .arg(name)
        .args(features(project))
        .arg("--quiet")
        .arg("--color=never")
        .arg("--message-format=json")
        .output()
        .map_err(Error::Cargo)
}

pub(crate) fn build_all_tests(project: &Project) -> Result<Output> {
    let _ = cargo(project)
        .arg("clean")
        .arg("--package")
        .arg(&project.name)
        .arg("--color=never")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    cargo(project)
        .arg("check")
        .args(target())
        .arg("--bins")
        .args(features(project))
        .arg("--quiet")
        .arg("--color=never")
        .arg("--message-format=json")
        .arg("--keep-going")
        .output()
        .map_err(Error::Cargo)
}

pub(crate) fn metadata() -> Result<Metadata> {
    let output = raw_cargo()
        .arg("metadata")
        .arg("--no-deps")
        .arg("--format-version=1")
        .output()
        .map_err(Error::Cargo)?;

    serde_json::from_slice(&output.stdout).map_err(|err| {
        print_col!("{}", String::from_utf8_lossy(&output.stderr));
        Error::Metadata(err)
    })
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
