use std::{io, path::PathBuf};

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("failed to execute cargo: {0}")]
    Cargo(io::Error),
    #[error("cargo reported an error")]
    CargoFail,
    #[error("failed to read manifest {0}: {1}")]
    GetManifest(PathBuf, Box<Error>),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("failed to read cargo metadata: {0}")]
    Metadata(#[from] cargo_metadata::Error),
    #[error("compiler error does not match expected error")]
    Mismatch,
    #[error("Cargo.toml uses edition.workspace=true, but no edition found in workspace's manifest")]
    NoWorkspaceManifest,
    #[error("{1}: {0}")]
    Open(PathBuf, io::Error),
    #[error("failed to determine name of project dir")]
    ProjectDir,
    #[error("expected test case to fail to compile, but it succeeded")]
    ShouldNotHaveCompiled,
    #[error(transparent)]
    TomlDe(#[from] toml::de::Error),
    #[error(transparent)]
    TomlSer(#[from] toml::ser::Error),
    #[error("unrecognized value of ERR_SPAN_CHECK: {0:?}")]
    UpdateVar(String),
    #[error("could not find or access tests/fail directory relative to {0}")]
    NoFailDir(PathBuf),
    #[error("Error searching tests/fail directory: {0}")]
    FailDirSearch(#[from] walkdir::Error),
    #[error("Failed to parse test case from {0}:{1}: {2}")]
    TestCaseParse(PathBuf, usize, String),
    #[error(
        r#"Invalid filename: {0:?}. Expected "<valid-nonempty-utf8>.rs"
Note that the tests/fail directory is only allowed to contain compile-fail test files."#
    )]
    InvalidFilename(PathBuf),
}

pub(crate) type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn already_printed(&self) -> bool {
        use self::Error::*;

        matches!(self, CargoFail | Mismatch | ShouldNotHaveCompiled)
    }
}
