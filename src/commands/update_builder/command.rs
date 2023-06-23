use crate::buildpack_info::{calculate_digest, BuildpackInfo, CalculateDigestError};
use crate::update_builder::errors::Error;
use clap::Parser;
use libcnb_package::{find_buildpack_dirs, read_buildpack_data};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use toml_edit::{value, Document};

type Result<T> = std::result::Result<T, Error>;

#[derive(Parser, Debug)]
#[command(author, version, about = "Updates all references to a buildpack in heroku/builder for the given list of builders", long_about = None)]
pub(crate) struct UpdateBuilderArgs {
    #[arg(long)]
    pub(crate) repository_path: String,
    #[arg(long)]
    pub(crate) builder_repository_path: String,
    #[arg(long, required = true, value_delimiter = ',', num_args = 1..)]
    pub(crate) builders: Vec<String>,
}

struct BuilderFile {
    path: PathBuf,
    document: Document,
}

pub(crate) fn execute(args: UpdateBuilderArgs) -> Result<()> {
    let current_dir = std::env::current_dir().map_err(Error::GetCurrentDir)?;
    let repository_path = resolve_path(PathBuf::from(args.repository_path), &current_dir);
    let builder_repository_path =
        resolve_path(PathBuf::from(args.builder_repository_path), &current_dir);

    let buildpacks = find_buildpack_dirs(&repository_path, &[repository_path.join("target")])
        .map_err(|e| Error::FindingBuildpacks(current_dir.clone(), e))?
        .into_iter()
        .map(|dir| read_buildpack_data(&dir).map_err(Error::ReadingBuildpackData))
        .collect::<Result<Vec<_>>>()?;

    if buildpacks.is_empty() {
        Err(Error::NoBuildpacks(repository_path))?;
    }

    let builder_files = args
        .builders
        .iter()
        .map(|builder| {
            read_builder_file(builder_repository_path.join(builder).join("builder.toml"))
        })
        .collect::<Result<Vec<_>>>()?;

    if builder_files.is_empty() {
        Err(Error::NoBuilderFiles(args.builders))?;
    }

    for mut builder_file in builder_files {
        for buildpack in &buildpacks {
            update_builder_with_buildpack_info(&mut builder_file.document, buildpack)
                .map_err(|e| Error::UpdatingBuilder(builder_file.path.clone(), e))?;
        }
        std::fs::write(&builder_file.path, builder_file.document.to_string())
            .map_err(|e| Error::WritingBuilder(builder_file.path.clone(), e))?;
        eprintln!("✅️ Updated builder: {}", builder_file.path.display());
    }

    Ok(())
}

fn resolve_path(path: PathBuf, current_dir: &Path) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        current_dir.join(path)
    }
}

fn read_builder_file(path: PathBuf) -> Result<BuilderFile> {
    let contents =
        std::fs::read_to_string(&path).map_err(|e| Error::ReadingBuilder(path.clone(), e))?;
    let document =
        Document::from_str(&contents).map_err(|e| Error::ParsingBuilder(path.clone(), e))?;
    Ok(BuilderFile { path, document })
}

#[derive(Debug)]
pub(crate) enum UpdateBuilderError {
    BuilderMissingRequiredKey(String),
    MissingDockerRepositoryMetadata(PathBuf),
    CalculatingDigest(PathBuf, CalculateDigestError),
}

fn update_builder_with_buildpack_info(
    document: &mut Document,
    buildpack_info: &dyn BuildpackInfo,
) -> std::result::Result<(), UpdateBuilderError> {
    let buildpack_id = buildpack_info.buildpack_id();
    let buildpack_version = buildpack_info.buildpack_version();
    let docker_repository = buildpack_info.docker_repository().ok_or(
        UpdateBuilderError::MissingDockerRepositoryMetadata(buildpack_info.path()),
    )?;
    let digest_url = format!("{docker_repository}:{buildpack_version}");
    let digest = calculate_digest(&digest_url)
        .map_err(|e| UpdateBuilderError::CalculatingDigest(buildpack_info.path(), e))?;
    let buildpack_uri = format!("docker://{docker_repository}@{digest}");

    document
        .get_mut("buildpacks")
        .and_then(|value| value.as_array_of_tables_mut())
        .unwrap_or(&mut toml_edit::ArrayOfTables::default())
        .iter_mut()
        .for_each(|buildpack| {
            let matches_id = buildpack
                .get("id")
                .and_then(|item| item.as_str())
                .filter(|value| value == &buildpack_id.as_str())
                .is_some();
            if matches_id {
                buildpack["uri"] = value(buildpack_uri.to_string());
            }
        });

    let order_list = document
        .get_mut("order")
        .and_then(|value| value.as_array_of_tables_mut())
        .ok_or(UpdateBuilderError::BuilderMissingRequiredKey(
            "order".to_string(),
        ))?;

    for order in order_list.iter_mut() {
        let group_list = order
            .get_mut("group")
            .and_then(|value| value.as_array_of_tables_mut())
            .ok_or(UpdateBuilderError::BuilderMissingRequiredKey(
                "group".to_string(),
            ))?;

        for group in group_list.iter_mut() {
            let matches_id = group
                .get("id")
                .and_then(|item| item.as_str())
                .filter(|value| value == &buildpack_id.as_str())
                .is_some();
            if matches_id {
                group["version"] = value(buildpack_version.to_string());
            }
        }
    }

    Ok(())
}

// #[cfg(test)]
// mod test {
//     use crate::commands::update_builder::command::{
//         update_builder_contents_with_buildpack, BuilderFile,
//     };
//     use libcnb_data::buildpack::BuildpackVersion;
//     use libcnb_data::buildpack_id;
//     use std::path::PathBuf;
//     use std::str::FromStr;
//     use toml_edit::Document;
//     use uriparse::URIReference;
//
//     #[test]
//     fn test_update_builder_contents_with_buildpack() {
//         let toml = r#"
// [[buildpacks]]
//   id = "heroku/java"
//   uri = "docker://docker.io/heroku/buildpack-java@sha256:21990393c93927b16f76c303ae007ea7e95502d52b0317ca773d4cd51e7a5682"
//
// [[buildpacks]]
//   id = "heroku/nodejs"
//   uri = "docker://docker.io/heroku/buildpack-nodejs@sha256:22ec91eebee2271b99368844f193c4bb3c6084201062f89b3e45179b938c3241"
//
// [[order]]
//   [[order.group]]
//     id = "heroku/nodejs"
//     version = "0.6.5"
//
// [[order]]
//   [[order.group]]
//     id = "heroku/java"
//     version = "0.6.9"
//
//   [[order.group]]
//     id = "heroku/procfile"
//     version = "2.0.0"
//     optional = true
// "#;
//         let mut builder_file = BuilderFile {
//             path: PathBuf::from("/path/to/builder.toml"),
//             document: Document::from_str(toml).unwrap(),
//         };
//         assert_eq!(
//             update_builder_contents_with_buildpack(
//                 &mut builder_file,
//                 &buildpack_id!("heroku/java"),
//                 &BuildpackVersion::try_from("0.6.10".to_string()).unwrap(),
//                 &URIReference::try_from("docker://docker.io/heroku/buildpack-java@sha256:c6dd500be06a2a1e764c30359c5dd4f4955a98b572ef3095b2f6115cd8a87c99").unwrap()
//             ).unwrap(),
//             r#"
// [[buildpacks]]
//   id = "heroku/java"
//   uri = "docker://docker.io/heroku/buildpack-java@sha256:c6dd500be06a2a1e764c30359c5dd4f4955a98b572ef3095b2f6115cd8a87c99"
//
// [[buildpacks]]
//   id = "heroku/nodejs"
//   uri = "docker://docker.io/heroku/buildpack-nodejs@sha256:22ec91eebee2271b99368844f193c4bb3c6084201062f89b3e45179b938c3241"
//
// [[order]]
//   [[order.group]]
//     id = "heroku/nodejs"
//     version = "0.6.5"
//
// [[order]]
//   [[order.group]]
//     id = "heroku/java"
//     version = "0.6.10"
//
//   [[order.group]]
//     id = "heroku/procfile"
//     version = "2.0.0"
//     optional = true
// "#
//         )
//     }
// }
