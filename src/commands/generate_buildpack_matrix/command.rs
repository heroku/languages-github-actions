use crate::buildpacks::{
    find_releasable_buildpacks, read_buildpack_descriptor, read_image_repository_metadata,
};
use crate::commands::generate_buildpack_matrix::errors::Error;
use crate::commands::resolve_path;
use crate::github::actions;
use clap::Parser;
use libcnb_data::buildpack::{BuildpackDescriptor, BuildpackId, BuildpackTarget};
use libcnb_package::output::{
    create_packaged_buildpack_dir_resolver, default_buildpack_directory_name,
};
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
        .filter_map(|t| rust_triple(&t).ok())
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
    os: Option<String>,
    arch: Option<String>,
    rust_triple: Option<String>,
    cnb_file: String,
    permanent_tag: String,
    temporary_tag: String,
    output_dir: PathBuf,
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
    let targets = read_buildpack_targets(buildpack_descriptor);
    Ok(BuildpackInfo {
        buildpack_id: buildpack_descriptor.buildpack().id.to_string(),
        buildpack_version: version.clone(),
        buildpack_dir: buildpack_dir.into(),
        targets: read_buildpack_targets(buildpack_descriptor)
            .iter()
            .map(|target| {
                let target_suffix = if targets.len() > 1 {
                    Some(target_name(target))
                } else {
                    None
                };
                Ok(TargetInfo {
                    cnb_file: cnb_file(
                        &buildpack_descriptor.buildpack().id,
                        target_suffix.as_deref(),
                    ),
                    os: target.os.clone(),
                    arch: target.arch.clone(),
                    output_dir: target_output_dir(
                        &buildpack_descriptor.buildpack().id,
                        buildpack_dir,
                        package_dir,
                        target,
                    )?,
                    rust_triple: rust_triple(target).ok(),
                    permanent_tag: generate_tag(&base_tag, &version, target_suffix.as_deref()),
                    temporary_tag: generate_tag(
                        &base_tag,
                        &format!("_{temporary_id}"),
                        target_suffix.as_deref(),
                    ),
                })
            })
            .collect::<Result<Vec<_>>>()?,
        permanent_tag: generate_tag(&base_tag, &version, None),
        temporary_tag: generate_tag(&base_tag, &format!("_{temporary_id}"), None),
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

fn generate_tag(base: &str, tag: &str, suffix: Option<&str>) -> String {
    suffix.map_or_else(
        || format!("{base}:{tag}"),
        |suffix| format!("{base}:{tag}_{suffix}"),
    )
}

fn cnb_file(buildpack_id: &BuildpackId, suffix: Option<&str>) -> String {
    let name = default_buildpack_directory_name(buildpack_id);
    suffix.map_or_else(
        || format!("{name}.cnb"),
        |suffix| format!("{name}_{suffix}.cnb"),
    )
}

fn target_name(target: &BuildpackTarget) -> String {
    match (target.os.as_deref(), target.arch.as_deref()) {
        (Some(os), Some(arch)) => format!("{os}-{arch}"),
        (Some(os), None) => os.to_string(),
        (None, Some(arch)) => format!("universal-{arch}"),
        (_, _) => "universal".to_string(),
    }
}

fn rust_triple(target: &BuildpackTarget) -> Result<String> {
    match (target.os.as_deref(), target.arch.as_deref()) {
        (Some("linux"), Some("amd64")) => Ok(String::from("x86_64-unknown-linux-musl")),
        (Some("linux"), Some("arm64")) => Ok(String::from("aarch64-unknown-linux-musl")),
        (_, _) => Err(Error::UnknownRustTarget(target.clone())),
    }
}

fn target_output_dir(
    buildpack_id: &BuildpackId,
    buildpack_dir: &Path,
    package_dir: &Path,
    target: &BuildpackTarget,
) -> Result<PathBuf> {
    if is_dynamic_buildpack(buildpack_dir) && !is_libcnb_buildpack(buildpack_dir) {
        return Ok(buildpack_dir.into());
    }
    Ok(create_packaged_buildpack_dir_resolver(
        package_dir,
        CargoProfile::Release,
        &rust_triple(target)?,
    )(buildpack_id))
}

fn is_libcnb_buildpack(buildpack_dir: &Path) -> bool {
    ["buildpack.toml", "Cargo.toml"]
        .iter()
        .all(|file| buildpack_dir.join(file).exists())
}

fn is_dynamic_buildpack(buildpack_dir: &Path) -> bool {
    ["detect", "build"]
        .iter()
        .all(|file| buildpack_dir.join("bin").join(file).exists())
}
