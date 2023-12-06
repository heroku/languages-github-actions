use crate::buildpacks::{
    find_releasable_buildpacks, read_buildpack_descriptor, read_image_repository_metadata,
};
use crate::commands::generate_buildpack_matrix::errors::Error;
use crate::commands::resolve_path;
use crate::github::actions;
use clap::Parser;
use libcnb_data::buildpack::{BuildpackDescriptor, Target};
use libcnb_package::output::{
    create_packaged_buildpack_dir_resolver, default_buildpack_directory_name,
};
use libcnb_package::CargoProfile;
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

type Result<T> = std::result::Result<T, Error>;

#[derive(Parser, Debug)]
#[command(author, version, about = "Generates a JSON list of buildpack information for each buildpack detected", long_about = None)]
pub(crate) struct GenerateBuildpackMatrixArgs {
    #[arg(long)]
    pub(crate) package_dir: PathBuf,
}

pub(crate) fn execute(args: &GenerateBuildpackMatrixArgs) -> Result<()> {
    let current_dir = std::env::current_dir().map_err(Error::GetCurrentDir)?;
    let package_dir = resolve_path(&args.package_dir, &current_dir);

    let buildpack_dirs =
        find_releasable_buildpacks(&current_dir).map_err(Error::FindReleasableBuildpacks)?;

    let buildpacks = buildpack_dirs
        .iter()
        .map(|dir| read_buildpack_descriptor(dir).map_err(Error::ReadBuildpackDescriptor))
        .collect::<Result<Vec<_>>>()?;

    let includes = buildpack_dirs
        .iter()
        .zip(buildpacks.iter())
        .map(|(buildpack_dir, buildpack_descriptor)| {
            extract_buildpack_info(buildpack_descriptor, buildpack_dir, &package_dir)
        })
        .collect::<Result<Vec<_>>>()?;

    let includes_json = serde_json::to_string(&includes).map_err(Error::SerializingJson)?;

    actions::set_output("buildpacks", includes_json).map_err(Error::SetActionOutput)?;

    let versions = buildpacks
        .iter()
        .map(|buildpack_descriptor| buildpack_descriptor.buildpack().version.to_string())
        .collect::<HashSet<_>>();

    if versions.len() != 1 {
        Err(Error::FixedVersion(versions.clone()))?;
    }

    let version = versions
        .iter()
        .next()
        .ok_or(Error::FixedVersion(versions.clone()))?;

    actions::set_output("version", version).map_err(Error::SetActionOutput)?;

    let rust_triples = buildpacks
        .iter()
        .flat_map(read_buildpack_targets)
        .map(|t| rust_triple(&t))
        .collect::<HashSet<String>>();

    actions::set_output(
        "rust_triples",
        serde_json::to_string(&rust_triples).map_err(Error::SerializingJson)?,
    )
    .map_err(Error::SetActionOutput)?;

    Ok(())
}

pub(crate) fn extract_buildpack_info(
    buildpack_descriptor: &BuildpackDescriptor,
    buildpack_dir: &Path,
    package_dir: &Path,
) -> Result<BTreeMap<String, serde_json::Value>> {
    Ok(BTreeMap::from([
        (
            "buildpack_id".to_string(),
            serde_json::Value::String(buildpack_descriptor.buildpack().id.to_string()),
        ),
        (
            "buildpack_version".to_string(),
            serde_json::Value::String(buildpack_descriptor.buildpack().version.to_string()),
        ),
        (
            "buildpack_dir".to_string(),
            serde_json::Value::String(buildpack_dir.to_string_lossy().to_string()),
        ),
        (
            "buildpack_artifact_prefix".to_string(),
            serde_json::Value::String(default_buildpack_directory_name(
                &buildpack_descriptor.buildpack().id,
            )),
        ),
        (
            "docker_repository".to_string(),
            serde_json::Value::String(read_image_repository_metadata(buildpack_descriptor).ok_or(
                Error::MissingImageRepositoryMetadata(buildpack_dir.join("buildpack.toml")),
            )?),
        ),
        (
            "targets".to_string(),
            extract_target_data(buildpack_descriptor, package_dir),
        ),
    ]))
}

// returns data for each target in the buildpack as a json array of objects,
// e.g. [ { "oci_platform": "linux/amd64", "rust_triple": "x86_64-unknown-linux-musl", "output_dir": "/somedir/x86_64-unknown-linux-musl/release/somebuildpack/" } ]
fn extract_target_data(
    buildpack_descriptor: &BuildpackDescriptor,
    package_dir: &Path,
) -> serde_json::Value {
    serde_json::Value::Array(
        read_buildpack_targets(buildpack_descriptor)
            .iter()
            .map(|target| {
                let triple = rust_triple(target);
                serde_json::Value::Object(serde_json::Map::from_iter([
                    (
                        "oci_platform".to_string(),
                        serde_json::Value::String(oci_platform(target)),
                    ),
                    (
                        "output_dir".to_string(),
                        serde_json::Value::String(
                            create_packaged_buildpack_dir_resolver(
                                package_dir,
                                CargoProfile::Release,
                                &triple,
                            )(&buildpack_descriptor.buildpack().id)
                            .to_string_lossy()
                            .to_string(),
                        ),
                    ),
                    ("rust_triple".to_string(), serde_json::Value::String(triple)),
                ]))
            })
            .collect(),
    )
}

// Reads targets from buildpacks while ensuring each buildpack returns at least
// one target (libcnb assumes a linux/amd64 target by default, even if no
// targets are defined).
fn read_buildpack_targets(buildpack_descriptor: &BuildpackDescriptor) -> Vec<Target> {
    let mut targets = match buildpack_descriptor {
        BuildpackDescriptor::Component(descriptor) => descriptor.targets.clone(),
        BuildpackDescriptor::Composite(_) => vec![],
    };
    if targets.is_empty() {
        targets.push(Target {
            os: Some("linux".into()),
            arch: Some("amd64".into()),
            variant: None,
            distros: vec![],
        });
    };
    targets
}

fn oci_platform(target: &Target) -> String {
    match (target.os.as_deref(), target.arch.as_deref()) {
        (Some(os), Some(arch)) => format!("{os}/{arch}"),
        (Some(os), None) => os.to_string(),
        (_, _) => String::new(),
    }
}
fn rust_triple(target: &Target) -> String {
    match (target.os.as_deref(), target.arch.as_deref()) {
        (Some("linux"), Some("amd64")) => String::from("x86_64-unknown-linux-musl"),
        (Some("linux"), Some("arm64")) => String::from("aarch64-unknown-linux-musl"),
        (_, _) => String::from("unknown-triple"),
    }
}
