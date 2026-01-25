use super::*;

#[derive(Serialize, Debug)]
pub(crate) struct Manifest {
    #[serde(rename = "cargo-features", skip_serializing_if = "Vec::is_empty")]
    pub cargo_features: Vec<String>,
    pub package: Package,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub features: BTreeMap<String, Vec<String>>,
    pub dependencies: BTreeMap<String, Dependency>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub target: BTreeMap<String, TargetDependencies>,
    #[serde(rename = "bin")]
    pub bins: Vec<Bin>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace: Option<Workspace>,
    #[serde(
        serialize_with = "serialize_patch",
        skip_serializing_if = "empty_patch"
    )]
    pub patch: BTreeMap<String, RegistryPatch>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub replace: BTreeMap<String, Patch>,
}

#[derive(Serialize, Debug)]
pub(crate) struct Package {
    pub name: String,
    pub version: String,
    pub edition: Edition,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolver: Option<String>,
    pub publish: bool,
}

#[derive(Serialize, Debug)]
pub(crate) struct Bin {
    pub name: String,
    pub path: PathBuf,
}

#[derive(Serialize, Debug)]
pub(crate) struct Workspace {
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub dependencies: BTreeMap<String, Dependency>,
}

impl Serialize for Dependency {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        Dependency::serialize(self, serializer)
    }
}

fn serialize_patch<S>(
    patch: &BTreeMap<String, RegistryPatch>,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error>
where
    S: Serializer,
{
    use serde::ser::SerializeMap;
    let mut map = serializer.serialize_map(None)?;
    for (registry, patch) in patch {
        if !patch.crates.is_empty() {
            map.serialize_entry(registry, patch)?;
        }
    }
    map.end()
}

fn empty_patch(patch: &BTreeMap<String, RegistryPatch>) -> bool {
    patch
        .values()
        .all(|registry_patch| registry_patch.crates.is_empty())
}
