use crate::{
    manifest::{generated, parsed},
    *,
};

#[derive(Debug)]
pub(crate) struct Project {
    pub dir: PathBuf,
    pub owned_dir: PathBuf,
    pub target_dir: PathBuf,
    pub name: String,
    pub should_update: bool,
    pub features: Option<Vec<String>>,
    pub workspace: PathBuf,
    pub path_dependencies: Vec<PathDependency>,
}

#[derive(Debug)]
pub(crate) struct PathDependency {
    pub name: String,
    pub normalized_path: PathBuf,
}

impl Project {
    pub(crate) fn prepare() -> Result<Self> {
        let cargo_metadata::Metadata {
            target_directory,
            workspace_root,
            packages,
            ..
        } = cargo::metadata()?;

        let source_dir = cargo::manifest_dir()?;
        let source_manifest = parsed::get_manifest(&source_dir)?;

        let mut features = util::features::find();

        let path_dependencies = source_manifest
            .dependencies
            .iter()
            .filter_map(|(name, dep)| {
                let path = dep.path.as_ref()?;
                if packages.iter().any(|p| p.name == name) {
                    // Skip path dependencies coming from the workspace itself
                    None
                } else {
                    Some(PathDependency {
                        name: name.clone(),
                        normalized_path: path.canonicalize().ok()?,
                    })
                }
            })
            .collect();

        let crate_name = &source_manifest.package.name;
        // A directory that we can use for our output.
        // Note: This directory is technically owned by trybuild. However, we have to use this exact path because
        //       other crates like llvm-cov have hardcoded workarounds for trybuild using this path.
        let owned_dir = target_directory.join("tests").join("trybuild");
        let project_dir = owned_dir.join(crate_name);
        std::fs::create_dir_all(&project_dir).context("failed to create project directory")?;

        let project_name = format!("{}-tests", crate_name);
        let manifest = Self::make_manifest(
            workspace_root.as_std_path(),
            &project_name,
            &source_dir,
            &packages,
            source_manifest,
        )?;

        if let Some(enabled_features) = &mut features {
            enabled_features.retain(|feature| manifest.features.contains_key(feature));
        }

        let manifest_toml =
            toml::to_string(&manifest).context("failed to serialize manifest to TOML")?;
        std::fs::write(project_dir.join("Cargo.toml"), manifest_toml)
            .context("failed to write Cargo.toml")?;

        let main_rs = b"\
            #![allow(unused_crate_dependencies, missing_docs)]\n\
            fn main() {}\n\
        ";
        std::fs::write(project_dir.join("main.rs"), main_rs).context("failed to write main.rs")?;

        let mut project = Project {
            dir: project_dir.into_std_path_buf(),
            owned_dir: owned_dir.into_std_path_buf(),
            target_dir: target_directory.into_std_path_buf(),
            name: project_name,
            should_update: util::env::should_update()?,
            features,
            workspace: workspace_root.into_std_path_buf(),
            path_dependencies,
        };

        cargo::build_dependencies(&mut project)?;

        Ok(project)
    }

    fn make_manifest(
        workspace: &Path,
        project_name: &str,
        source_dir: &Path,
        packages: &[cargo_metadata::Package],
        source_manifest: parsed::Manifest,
    ) -> Result<generated::Manifest> {
        use manifest::{Dependency, EditionOrInherit};

        let crate_name = source_manifest.package.name;
        let workspace_manifest = parsed::get_workspace_manifest(workspace);

        let edition = match source_manifest.package.edition {
            EditionOrInherit::Edition(edition) => edition,
            EditionOrInherit::Inherit => workspace_manifest
                .workspace
                .package
                .edition
                .context("Cargo.toml uses edition.workspace=true, but no edition found in workspace's manifest")?,
        };

        let mut dependencies = BTreeMap::new();
        dependencies.extend(source_manifest.dependencies);
        dependencies.extend(source_manifest.dev_dependencies);

        let cargo_toml_path = source_dir.join("Cargo.toml");
        let mut has_lib_target = true;
        for package_metadata in packages {
            if package_metadata.manifest_path == cargo_toml_path {
                has_lib_target = package_metadata
                    .targets
                    .iter()
                    .any(|target| target.crate_types != [cargo_metadata::CrateType::Bin]);
            }
        }
        if has_lib_target {
            dependencies.insert(
                crate_name.clone(),
                Dependency {
                    version: None,
                    path: Some(source_dir.to_path_buf()),
                    optional: false,
                    default_features: Some(false),
                    features: Vec::new(),
                    git: None,
                    branch: None,
                    tag: None,
                    rev: None,
                    workspace: false,
                    rest: BTreeMap::new(),
                },
            );
        }

        let mut targets = source_manifest.target;
        for target in targets.values_mut() {
            let dev_dependencies = std::mem::take(&mut target.dev_dependencies);
            target.dependencies.extend(dev_dependencies);
        }

        let mut features = source_manifest.features;
        for (feature, enables) in &mut features {
            enables.retain(|en| {
                let Some(dep_name) = en.strip_prefix("dep:") else {
                    return false;
                };
                if let Some(Dependency { optional: true, .. }) = dependencies.get(dep_name) {
                    return true;
                }
                for target in targets.values() {
                    if let Some(Dependency { optional: true, .. }) =
                        target.dependencies.get(dep_name)
                    {
                        return true;
                    }
                }
                false
            });
            if has_lib_target {
                enables.insert(0, format!("{}/{}", crate_name, feature));
            }
        }

        let mut manifest = generated::Manifest {
            cargo_features: source_manifest.cargo_features,
            package: generated::Package {
                name: project_name.to_owned(),
                version: "0.0.0".to_owned(),
                edition,
                resolver: source_manifest.package.resolver,
                publish: false,
            },
            features,
            dependencies,
            target: targets,
            bins: Vec::new(),
            workspace: Some(generated::Workspace {
                dependencies: workspace_manifest.workspace.dependencies,
            }),
            // Within a workspace, only the [patch] and [replace] sections in
            // the workspace root's Cargo.toml are applied by Cargo.
            patch: workspace_manifest.patch,
            replace: workspace_manifest.replace,
        };

        manifest.bins.push(generated::Bin {
            name: project_name.to_owned(),
            path: Path::new("main.rs").to_owned(),
        });

        Ok(manifest)
    }
}
