use super::*;

use serde::de::{
    self,
    value::{MapAccessDeserializer, StrDeserializer},
};

pub(crate) fn get_manifest(manifest_dir: &Path) -> Result<Manifest> {
    let cargo_toml_path = manifest_dir.join("Cargo.toml");

    let manifest_str = std::fs::read_to_string(&cargo_toml_path)
        .path_context(&cargo_toml_path, "failed to read manifest: <path>")?;

    let mut manifest: Manifest = toml::from_str(&manifest_str)
        .path_context(&cargo_toml_path, "failed to parse manifest: <path>")?;

    fix_dependencies(&mut manifest.dependencies, manifest_dir);
    fix_dependencies(&mut manifest.dev_dependencies, manifest_dir);
    for target in manifest.target.values_mut() {
        fix_dependencies(&mut target.dependencies, manifest_dir);
        fix_dependencies(&mut target.dev_dependencies, manifest_dir);
    }

    Ok(manifest)
}

pub(crate) fn get_workspace_manifest(manifest_dir: &Path) -> WorkspaceManifest {
    try_get_workspace_manifest(manifest_dir).unwrap_or_default()
}

fn try_get_workspace_manifest(manifest_dir: &Path) -> Result<WorkspaceManifest> {
    let cargo_toml_path = manifest_dir.join("Cargo.toml");
    let manifest_str = std::fs::read_to_string(cargo_toml_path)?;
    let mut manifest: WorkspaceManifest = toml::from_str(&manifest_str)?;

    fix_dependencies(&mut manifest.workspace.dependencies, manifest_dir);
    fix_patches(&mut manifest.patch, manifest_dir);
    fix_replacements(&mut manifest.replace, manifest_dir);

    Ok(manifest)
}

fn fix_dependencies(dependencies: &mut BTreeMap<String, Dependency>, dir: &Path) {
    dependencies.remove("err_span_check");
    for dep in dependencies.values_mut() {
        dep.path = dep.path.as_ref().map(|path| dir.join(path));
    }
}

fn fix_patches(patches: &mut BTreeMap<String, RegistryPatch>, dir: &Path) {
    for registry in patches.values_mut() {
        registry.crates.remove("err_span_check");
        for patch in registry.crates.values_mut() {
            patch.path = patch.path.as_ref().map(|path| dir.join(path));
        }
    }
}

fn fix_replacements(replacements: &mut BTreeMap<String, Patch>, dir: &Path) {
    replacements.remove("err_span_check");
    for replacement in replacements.values_mut() {
        replacement.path = replacement.path.as_ref().map(|path| dir.join(path));
    }
}

#[derive(Deserialize, Default, Debug)]
pub(crate) struct WorkspaceManifest {
    #[serde(default)]
    pub workspace: WorkspaceWorkspace,
    #[serde(default)]
    pub patch: BTreeMap<String, RegistryPatch>,
    #[serde(default)]
    pub replace: BTreeMap<String, Patch>,
}

#[derive(Deserialize, Default, Debug)]
pub(crate) struct WorkspaceWorkspace {
    #[serde(default)]
    pub package: WorkspacePackage,
    #[serde(default)]
    pub dependencies: BTreeMap<String, Dependency>,
}

#[derive(Deserialize, Default, Debug)]
pub(crate) struct WorkspacePackage {
    pub edition: Option<Edition>,
}

#[derive(Deserialize, Default, Debug)]
pub(crate) struct Manifest {
    #[serde(rename = "cargo-features", default)]
    pub cargo_features: Vec<String>,
    #[serde(default)]
    pub package: Package,
    #[serde(default)]
    pub features: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub dependencies: BTreeMap<String, Dependency>,
    #[serde(default, alias = "dev-dependencies")]
    pub dev_dependencies: BTreeMap<String, Dependency>,
    #[serde(default)]
    pub target: BTreeMap<String, TargetDependencies>,
}

#[derive(Deserialize, Default, Debug)]
pub(crate) struct Package {
    pub name: String,
    #[serde(default)]
    pub edition: EditionOrInherit,
    pub resolver: Option<String>,
}

impl<'de> Deserialize<'de> for EditionOrInherit {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct EditionOrInheritVisitor;

        impl<'de> de::Visitor<'de> for EditionOrInheritVisitor {
            type Value = EditionOrInherit;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("edition")
            }

            fn visit_str<E>(self, s: &str) -> std::result::Result<Self::Value, E>
            where
                E: de::Error,
            {
                Edition::deserialize(StrDeserializer::new(s)).map(EditionOrInherit::Edition)
            }

            fn visit_map<M>(self, map: M) -> std::result::Result<Self::Value, M::Error>
            where
                M: de::MapAccess<'de>,
            {
                InheritEdition::deserialize(MapAccessDeserializer::new(map))?;
                Ok(EditionOrInherit::Inherit)
            }
        }

        deserializer.deserialize_any(EditionOrInheritVisitor)
    }
}

impl<'de> Deserialize<'de> for Dependency {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct DependencyVisitor;

        impl<'de> de::Visitor<'de> for DependencyVisitor {
            type Value = Dependency;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str(
                    "a version string like \"0.9.8\" or a \
                     dependency like { version = \"0.9.8\" }",
                )
            }

            fn visit_str<E>(self, s: &str) -> std::result::Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Dependency {
                    version: Some(s.to_owned()),
                    path: None,
                    optional: false,
                    default_features: Some(true),
                    features: Vec::new(),
                    git: None,
                    branch: None,
                    tag: None,
                    rev: None,
                    workspace: false,
                    rest: BTreeMap::new(),
                })
            }

            fn visit_map<M>(self, map: M) -> std::result::Result<Self::Value, M::Error>
            where
                M: de::MapAccess<'de>,
            {
                Dependency::deserialize(MapAccessDeserializer::new(map))
            }
        }

        deserializer.deserialize_any(DependencyVisitor)
    }
}
