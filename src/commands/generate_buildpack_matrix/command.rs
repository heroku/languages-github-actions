use crate::buildpacks::{find_releasable_buildpacks, read_image_repository_metadata};
use crate::commands::generate_buildpack_matrix::errors::Error;
use crate::github::actions;
use clap::Parser;
use libcnb_package::{
    get_buildpack_target_dir, read_buildpack_data, BuildpackData, GenericMetadata,
};
use std::collections::{BTreeMap, HashSet};
use std::path::Path;

type Result<T> = std::result::Result<T, Error>;

#[derive(Parser, Debug)]
#[command(author, version, about = "Generates a JSON list of buildpack information for each buildpack detected", long_about = None)]
pub(crate) struct GenerateBuildpackMatrixArgs;

pub(crate) fn execute(_: GenerateBuildpackMatrixArgs) -> Result<()> {
    let current_dir = std::env::current_dir().map_err(Error::GetCurrentDir)?;

    let buildpack_dirs = find_releasable_buildpacks(&current_dir)
        .map_err(|e| Error::FindingBuildpacks(current_dir.clone(), e))?;

    let buildpacks = buildpack_dirs
        .iter()
        .map(|dir| read_buildpack_data(dir).map_err(Error::ReadingBuildpackData))
        .collect::<Result<Vec<_>>>()?;

    let includes = buildpack_dirs
        .iter()
        .zip(buildpacks.iter())
        .map(|(dir, data)| extract_buildpack_info(data, dir, &current_dir))
        .collect::<Result<Vec<_>>>()?;

    let includes_json = serde_json::to_string(&includes).map_err(Error::SerializingJson)?;

    actions::set_output("buildpacks", includes_json).map_err(Error::SetActionOutput)?;

    let versions = buildpacks
        .iter()
        .map(|data| data.buildpack_descriptor.buildpack().version.to_string())
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
    buildpack_data: &BuildpackData<GenericMetadata>,
    dir: &Path,
    workspace_dir: &Path,
) -> Result<BTreeMap<String, String>> {
    let buildpack_dir = dir.to_string_lossy().to_string();

    let buildpack_path = buildpack_data.buildpack_descriptor_path.clone();

    let buildpack_id = buildpack_data.buildpack_descriptor.buildpack().id.clone();

    let buildpack_version = buildpack_data
        .buildpack_descriptor
        .buildpack()
        .version
        .to_string();

    let buildpack_artifact_prefix = buildpack_id.replace('/', "_");

    let docker_repository = read_image_repository_metadata(&buildpack_data.buildpack_descriptor)
        .ok_or(Error::MissingDockerRepositoryMetadata(buildpack_path))?;

    let buildpack_output_dir = get_buildpack_target_dir(
        &buildpack_id,
        &workspace_dir.to_path_buf().join("target"),
        true,
    );

    Ok(BTreeMap::from([
        ("buildpack_id".to_string(), buildpack_id.to_string()),
        ("buildpack_version".to_string(), buildpack_version),
        ("buildpack_dir".to_string(), buildpack_dir),
        (
            "buildpack_artifact_prefix".to_string(),
            buildpack_artifact_prefix,
        ),
        (
            "buildpack_output_dir".to_string(),
            buildpack_output_dir.to_string_lossy().to_string(),
        ),
        ("docker_repository".to_string(), docker_repository),
    ]))
}
