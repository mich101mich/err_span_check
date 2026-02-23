use crate::*;

pub(crate) mod generated;
mod inherit;
pub(crate) mod parsed;

use inherit::InheritEdition;

#[derive(Serialize, Deserialize, Default, Debug)]
pub(crate) enum Edition {
    #[default]
    #[serde(rename = "2015")]
    E2015,
    #[serde(rename = "2018")]
    E2018,
    #[serde(rename = "2021")]
    E2021,
    #[serde(rename = "2024")]
    E2024,
}

#[derive(Debug)]
pub(crate) enum EditionOrInherit {
    Edition(Edition),
    Inherit,
}

impl Default for EditionOrInherit {
    fn default() -> Self {
        EditionOrInherit::Edition(Edition::default())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(remote = "Self")]
pub(crate) struct Dependency {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub optional: bool,
    #[serde(rename = "default-features", skip_serializing_if = "Option::is_none")]
    pub default_features: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rev: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub workspace: bool,
    #[serde(flatten)]
    pub rest: BTreeMap<String, serde_json::Value>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct TargetDependencies {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dependencies: BTreeMap<String, Dependency>,
    #[serde(
        default,
        alias = "dev-dependencies",
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub dev_dependencies: BTreeMap<String, Dependency>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(transparent)]
pub(crate) struct RegistryPatch {
    pub crates: BTreeMap<String, Patch>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct Patch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rev: Option<String>,
    #[serde(flatten)]
    pub rest: BTreeMap<String, serde_json::Value>,
}

fn is_false(boolean: &bool) -> bool {
    !*boolean
}
