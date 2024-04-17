use crate::buildpacks::{
    find_releasable_buildpacks, read_buildpack_descriptor, read_image_repository_metadata,
};
use crate::commands::generate_buildpack_matrix::errors::Error;
use crate::commands::resolve_path;
use crate::github::actions;
use clap::Parser;
use libcnb_data::buildpack::{BuildpackDescriptor, BuildpackTarget};
use libcnb_package::output::create_packaged_buildpack_dir_resolver;
use libcnb_package::CargoProfile;
use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

type Result<T> = std::result::Result<T, Error>;

#[derive(Parser, Debug)]
#[command(author, version, about = "Generates a JSON list of buildpack information for each buildpack detected", long_about = None)]
pub(crate) struct GenerateBuildpackMatrixArgs {
    #[arg(long)]
    pub(crate) source_dir: Option<PathBuf>,
    #[arg(long)]
    pub(crate) package_dir: Option<PathBuf>,
    #[arg(long)]
    pub(crate) temporary_id: String,
}

pub(crate) fn execute(args: &GenerateBuildpackMatrixArgs) -> Result<()> {
    let source_dir = match &args.source_dir {
        Some(path) => path.clone(),
        None => std::env::current_dir().map_err(Error::GetCurrentDir)?,
    };
    let package_dir = resolve_path(
        match &args.package_dir {
            Some(path) => path,
            None => Path::new("./packaged"),
        },
        &source_dir,
    );

    let buildpack_dirs =
        find_releasable_buildpacks(&source_dir).map_err(Error::FindReleasableBuildpacks)?;

    let buildpacks = buildpack_dirs
        .iter()
        .map(|dir| read_buildpack_descriptor(dir).map_err(Error::ReadBuildpackDescriptor))
        .collect::<Result<Vec<_>>>()?;

    let buildpacks_info = buildpack_dirs
        .iter()
        .zip(buildpacks.iter())
        .map(|(buildpack_dir, buildpack_descriptor)| {
            read_buildpack_info(
                buildpack_descriptor,
                buildpack_dir,
                &package_dir,
                &args.temporary_id,
            )
        })
        .collect::<Result<Vec<_>>>()?;

    let buildpacks_json =
        serde_json::to_string(&buildpacks_info).map_err(Error::SerializingJson)?;

    actions::set_output("buildpacks", buildpacks_json).map_err(Error::SetActionOutput)?;

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

#[derive(Serialize)]
pub(crate) struct BuildpackInfo {
    buildpack_id: String,
    buildpack_version: String,
    buildpack_dir: PathBuf,
    targets: Vec<TargetInfo>,
    permanent_tag: String,
    temporary_tag: String,
}

#[derive(Serialize)]
pub(crate) struct TargetInfo {
    rust_triple: String,
    oci_platform: String,
    output_dir: PathBuf,
    permanent_tag: String,
    temporary_tag: String,
}

pub(crate) fn read_buildpack_info(
    buildpack_descriptor: &BuildpackDescriptor,
    buildpack_dir: &Path,
    package_dir: &Path,
    temporary_id: &str,
) -> Result<BuildpackInfo> {
    let version = buildpack_descriptor.buildpack().version.to_string();
    let base_tag = read_image_repository_metadata(buildpack_descriptor).ok_or(
        Error::MissingImageRepositoryMetadata(buildpack_dir.join("buildpack.toml")),
    )?;
    Ok(BuildpackInfo {
        buildpack_id: buildpack_descriptor.buildpack().id.to_string(),
        buildpack_version: version.clone(),
        buildpack_dir: buildpack_dir.into(),
        targets: read_buildpack_targets(buildpack_descriptor)
            .iter()
            .map(|target| {
                let triple = rust_triple(target);
                TargetInfo {
                    oci_platform: oci_platform(target),
                    output_dir: create_packaged_buildpack_dir_resolver(
                        package_dir,
                        CargoProfile::Release,
                        &triple,
                    )(&buildpack_descriptor.buildpack().id),
                    rust_triple: triple,
                    permanent_tag: permanent_tag(&base_tag, &version, Some(target)),
                    temporary_tag: temporary_tag(&base_tag, temporary_id, Some(target)),
                }
            })
            .collect(),
        permanent_tag: permanent_tag(&base_tag, &version, None),
        temporary_tag: temporary_tag(&base_tag, temporary_id, None),
    })
}

// Reads targets from buildpacks while ensuring each buildpack returns at least
// one target (libcnb assumes a linux/amd64 target by default, even if no
// targets are defined).
fn read_buildpack_targets(buildpack_descriptor: &BuildpackDescriptor) -> Vec<BuildpackTarget> {
    let mut targets = match buildpack_descriptor {
        BuildpackDescriptor::Component(descriptor) => descriptor.targets.clone(),
        BuildpackDescriptor::Composite(_) => vec![],
    };
    if targets.is_empty() {
        targets.push(BuildpackTarget {
            os: Some("linux".into()),
            arch: Some("amd64".into()),
            variant: None,
            distros: vec![],
        });
    };
    targets
}

fn permanent_tag(base_tag: &str, version: &str, target: Option<&BuildpackTarget>) -> String {
    generate_tag(base_tag, version, target)
}

fn temporary_tag(base_tag: &str, temporary_id: &str, target: Option<&BuildpackTarget>) -> String {
    generate_tag(base_tag, &format!("_{temporary_id}"), target)
}

fn generate_tag(base_tag: &str, suffix: &str, target: Option<&BuildpackTarget>) -> String {
    if let Some(name) = target.map(target_name) {
        return format!("{base_tag}:{suffix}_{name}");
    }
    format!("{base_tag}:{suffix}")
}

fn target_name(target: &BuildpackTarget) -> String {
    match (target.os.as_deref(), target.arch.as_deref()) {
        (Some(os), Some(arch)) => format!("{os}-{arch}"),
        (Some(os), None) => os.to_string(),
        (None, Some(arch)) => format!("universal-{arch}"),
        (_, _) => "universal".to_string(),
    }
}

fn oci_platform(target: &BuildpackTarget) -> String {
    match (target.os.as_deref(), target.arch.as_deref()) {
        (Some(os), Some(arch)) => format!("{os}/{arch}"),
        (Some(os), None) => os.to_string(),
        (_, _) => String::new(),
    }
}
fn rust_triple(target: &BuildpackTarget) -> String {
    match (target.os.as_deref(), target.arch.as_deref()) {
        (Some("linux"), Some("amd64")) => String::from("x86_64-unknown-linux-musl"),
        (Some("linux"), Some("arm64")) => String::from("aarch64-unknown-linux-musl"),
        (_, _) => String::from("unknown-triple"),
    }
}
