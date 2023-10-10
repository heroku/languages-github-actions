use crate::buildpacks::{
    find_releasable_buildpacks, read_buildpack_descriptor, read_image_repository_metadata,
};
use crate::commands::generate_buildpack_matrix::errors::Error;
use crate::commands::resolve_path;
use crate::github::actions;
use clap::Parser;
use libcnb_data::buildpack::{BuildpackDescriptor, BuildpackId};
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
    pub(crate) working_dir: Option<PathBuf>,
    #[arg(long)]
    pub(crate) package_dir: PathBuf,
    #[arg(long)]
    pub(crate) release: Option<bool>,
    #[arg(long, default_value = "x86_64-unknown-linux-musl")]
    pub(crate) target: String,
}

pub(crate) fn execute(args: GenerateBuildpackMatrixArgs) -> Result<()> {
    let working_dir = std::env::current_dir()
        .map(|base| match args.working_dir {
            Some(path) => resolve_path(&path, &base),
            None => base,
        })
        .map_err(Error::ResolveWorkingDir)?;

    let package_dir = resolve_path(&args.package_dir, &working_dir);

    let cargo_profile = if args.release.unwrap_or(true) {
        CargoProfile::Release
    } else {
        CargoProfile::Dev
    };

    let packaged_buildpack_dir_resolver =
        create_packaged_buildpack_dir_resolver(&package_dir, cargo_profile, &args.target);

    let buildpack_dirs =
        find_releasable_buildpacks(&working_dir).map_err(Error::FindReleasableBuildpacks)?;

    let buildpacks = buildpack_dirs
        .iter()
        .map(|dir| read_buildpack_descriptor(dir).map_err(Error::ReadBuildpackDescriptor))
        .collect::<Result<Vec<_>>>()?;

    let includes = buildpack_dirs
        .iter()
        .zip(buildpacks.iter())
        .map(|(buildpack_dir, buildpack_descriptor)| {
            extract_buildpack_info(
                buildpack_descriptor,
                buildpack_dir,
                &packaged_buildpack_dir_resolver,
            )
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

    Ok(())
}

pub(crate) fn extract_buildpack_info(
    buildpack_descriptor: &BuildpackDescriptor,
    buildpack_dir: &Path,
    packaged_buildpack_dir_resolver: &impl Fn(&BuildpackId) -> PathBuf,
) -> Result<BTreeMap<String, String>> {
    Ok(BTreeMap::from([
        (
            "buildpack_id".to_string(),
            buildpack_descriptor.buildpack().id.to_string(),
        ),
        (
            "buildpack_version".to_string(),
            buildpack_descriptor.buildpack().version.to_string(),
        ),
        (
            "buildpack_dir".to_string(),
            buildpack_dir.to_string_lossy().to_string(),
        ),
        (
            "buildpack_artifact_prefix".to_string(),
            default_buildpack_directory_name(&buildpack_descriptor.buildpack().id),
        ),
        (
            "buildpack_output_dir".to_string(),
            packaged_buildpack_dir_resolver(&buildpack_descriptor.buildpack().id)
                .to_string_lossy()
                .to_string(),
        ),
        (
            "docker_repository".to_string(),
            read_image_repository_metadata(buildpack_descriptor).ok_or(
                Error::MissingDockerRepositoryMetadata(buildpack_dir.join("buildpack.toml")),
            )?,
        ),
    ]))
}
