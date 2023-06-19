use crate::commands::generate_buildpack_matrix::errors::Error;
use crate::github::actions;
use clap::Parser;
use libcnb_package::{find_buildpack_dirs, read_buildpack_data, FindBuildpackDirsOptions};
use std::collections::HashMap;

type Result<T> = std::result::Result<T, Error>;

#[derive(Parser, Debug)]
#[command(author, version, about = "Generates a JSON list of {id, path} entries for each buildpack detected", long_about = None)]
pub(crate) struct GenerateBuildpackMatrixArgs;

pub(crate) fn execute(_: GenerateBuildpackMatrixArgs) -> Result<()> {
    let current_dir = std::env::current_dir().map_err(Error::GetCurrentDir)?;

    let find_buildpack_dirs_options = FindBuildpackDirsOptions {
        ignore: vec![current_dir.join("target")],
    };

    let buildpacks = find_buildpack_dirs(&current_dir, &find_buildpack_dirs_options)
        .map_err(Error::FindingBuildpacks)?
        .into_iter()
        .map(|dir| {
            read_buildpack_data(&dir)
                .map_err(Error::ReadingBuildpackData)
                .map(|data| {
                    HashMap::from([
                        ("id", data.buildpack_descriptor.buildpack().id.to_string()),
                        ("path", dir.to_string_lossy().to_string()),
                    ])
                })
        })
        .collect::<Result<Vec<_>>>()?;

    let json = serde_json::to_string(&buildpacks).map_err(Error::SerializingJson)?;

    actions::set_output("buildpacks", json).map_err(Error::SetActionOutput)?;

    Ok(())
}
