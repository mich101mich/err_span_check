use glob::{GlobError, PatternError};
use std::io;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("failed to execute cargo: {0}")]
    Cargo(io::Error),
    #[error("cargo reported an error")]
    CargoFail,
    #[error("failed to read manifest {0}: {1}")]
    GetManifest(PathBuf, Box<Error>),
    #[error(transparent)]
    Glob(#[from] GlobError),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("failed to read cargo metadata: {0}")]
    Metadata(serde_json::Error),
    #[error("compiler error does not match expected error")]
    Mismatch,
    #[error("Cargo.toml uses edition.workspace=true, but no edition found in workspace's manifest")]
    NoWorkspaceManifest,
    #[error("{1}: {0}")]
    Open(PathBuf, io::Error),
    #[error(transparent)]
    Pattern(#[from] PatternError),
    #[error("failed to determine name of project dir")]
    ProjectDir,
    #[error("failed to read stderr file: {0}")]
    ReadStderr(io::Error),
    #[error("expected test case to fail to compile, but it succeeded")]
    ShouldNotHaveCompiled,
    #[error(transparent)]
    TomlDe(#[from] toml::de::Error),
    #[error(transparent)]
    TomlSer(#[from] toml::ser::Error),
    #[error("unrecognized value of ERR_SPAN_CHECK: {0:?}")]
    UpdateVar(String),
    #[error("failed to write stderr file: {0}")]
    WriteStderr(io::Error),
}

pub(crate) type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn already_printed(&self) -> bool {
        use self::Error::*;

        matches!(self, CargoFail | Mismatch | ShouldNotHaveCompiled)
    }
}
