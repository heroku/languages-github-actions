use crate::buildpacks::{
    calculate_digest, find_releasable_buildpacks, read_image_repository_metadata,
};
use crate::update_builder::errors::Error;
use clap::Parser;
use libcnb_data::buildpack::{BuildpackId, BuildpackVersion};
use libcnb_package::read_buildpack_data;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use toml_edit::{value, ArrayOfTables, Document, Item};
use uriparse::URI;

type Result<T> = std::result::Result<T, Error>;

#[derive(Parser, Debug)]
#[command(author, version, about = "Updates all references to a buildpack in heroku/cnb-builder-images for the given list of builders", long_about = None)]
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

    let buildpacks = find_releasable_buildpacks(&repository_path)
        .map_err(|e| Error::FindingBuildpacks(current_dir.clone(), e))?
        .into_iter()
        .map(|dir| read_buildpack_data(dir).map_err(Error::ReadingBuildpackData))
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
        for buildpack_data in &buildpacks {
            let buildpack_path = &buildpack_data.buildpack_descriptor_path;

            let buildpack_id = &buildpack_data.buildpack_descriptor.buildpack().id;

            let buildpack_version = &buildpack_data.buildpack_descriptor.buildpack().version;

            let docker_repository =
                read_image_repository_metadata(&buildpack_data.buildpack_descriptor).ok_or(
                    Error::MissingDockerRepositoryMetadata(buildpack_path.clone()),
                )?;

            let buildpack_uri =
                calculate_digest(&format!("{docker_repository}:{buildpack_version}"))
                    .map_err(|e| Error::CalculatingDigest(buildpack_path.clone(), e))
                    .map(|digest| format!("docker://{docker_repository}@{digest}"))?;

            update_builder_with_buildpack_info(
                &mut builder_file.document,
                buildpack_id,
                buildpack_version,
                &buildpack_uri,
            )?;
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

fn update_builder_with_buildpack_info(
    document: &mut Document,
    buildpack_id: &BuildpackId,
    buildpack_version: &BuildpackVersion,
    buildpack_uri_with_sha: &str,
) -> Result<()> {
    if is_buildpack_using_cnb_shim(document, buildpack_id) {
        return Ok(());
    }

    document
        .get_mut("buildpacks")
        .and_then(Item::as_array_of_tables_mut)
        .unwrap_or(&mut ArrayOfTables::default())
        .iter_mut()
        .for_each(|buildpack| {
            let matches_id = buildpack
                .get("id")
                .and_then(Item::as_str)
                .filter(|value| value == &buildpack_id.as_str())
                .is_some();
            if matches_id {
                buildpack["uri"] = value(buildpack_uri_with_sha.to_string());
            }
        });

    let order_list = document
        .get_mut("order")
        .and_then(Item::as_array_of_tables_mut)
        .ok_or(Error::BuilderMissingRequiredKey("order".to_string()))?;

    for order in order_list.iter_mut() {
        let group_list = order
            .get_mut("group")
            .and_then(Item::as_array_of_tables_mut)
            .ok_or(Error::BuilderMissingRequiredKey("group".to_string()))?;

        for group in group_list.iter_mut() {
            let matches_id = group
                .get("id")
                .and_then(Item::as_str)
                .filter(|value| value == &buildpack_id.as_str())
                .is_some();
            if matches_id {
                group["version"] = value(buildpack_version.to_string());
            }
        }
    }

    Ok(())
}

fn is_buildpack_using_cnb_shim(document: &Document, buildpack_id: &BuildpackId) -> bool {
    document
        .get("buildpacks")
        .and_then(Item::as_array_of_tables)
        .unwrap_or(&ArrayOfTables::default())
        .iter()
        .any(|buildpack| {
            let matches_id = buildpack
                .get("id")
                .and_then(Item::as_str)
                .filter(|value| value == &buildpack_id.as_str())
                .is_some();

            let uses_cnb_shim_url =
                buildpack
                    .get("uri")
                    .and_then(Item::as_str)
                    .into_iter()
                    .any(|uri| match URI::try_from(uri) {
                        Ok(parsed_uri) => parsed_uri
                            .host()
                            .map_or(false, |host| host.to_string() == "cnb-shim.herokuapp.com"),
                        Err(_) => false,
                    });

            matches_id && uses_cnb_shim_url
        })
}

#[cfg(test)]
mod test {
    use crate::commands::update_builder::command::update_builder_with_buildpack_info;
    use libcnb_data::buildpack::BuildpackVersion;
    use libcnb_data::buildpack_id;
    use std::str::FromStr;
    use toml_edit::Document;

    #[test]
    fn test_update_builder_contents_with_buildpack() {
        let toml = r#"
[[buildpacks]]
  id = "heroku/java"
  uri = "docker://docker.io/heroku/buildpack-java@sha256:21990393c93927b16f76c303ae007ea7e95502d52b0317ca773d4cd51e7a5682"

[[buildpacks]]
  id = "heroku/nodejs"
  uri = "docker://docker.io/heroku/buildpack-nodejs@sha256:22ec91eebee2271b99368844f193c4bb3c6084201062f89b3e45179b938c3241"

[[order]]
  [[order.group]]
    id = "heroku/nodejs"
    version = "0.6.5"

[[order]]
  [[order.group]]
    id = "heroku/java"
    version = "0.6.9"

  [[order.group]]
    id = "heroku/procfile"
    version = "2.0.0"
    optional = true
"#;
        let mut document = Document::from_str(toml).unwrap();

        update_builder_with_buildpack_info(
            &mut document,
            &buildpack_id!("heroku/java"),
            &BuildpackVersion::try_from("0.6.10".to_string()).unwrap(),
            "docker://docker.io/heroku/buildpack-java@sha256:some-java-test-sha",
        )
        .unwrap();

        update_builder_with_buildpack_info(
            &mut document,
            &buildpack_id!("heroku/nodejs"),
            &BuildpackVersion::try_from("0.6.6".to_string()).unwrap(),
            "docker://docker.io/heroku/buildpack-nodejs@sha256:some-nodejs-test-sha",
        )
        .unwrap();

        assert_eq!(
            document.to_string(),
            r#"
[[buildpacks]]
  id = "heroku/java"
  uri = "docker://docker.io/heroku/buildpack-java@sha256:some-java-test-sha"

[[buildpacks]]
  id = "heroku/nodejs"
  uri = "docker://docker.io/heroku/buildpack-nodejs@sha256:some-nodejs-test-sha"

[[order]]
  [[order.group]]
    id = "heroku/nodejs"
    version = "0.6.6"

[[order]]
  [[order.group]]
    id = "heroku/java"
    version = "0.6.10"

  [[order.group]]
    id = "heroku/procfile"
    version = "2.0.0"
    optional = true
"#
        );
    }

    #[test]
    fn test_update_builder_contents_does_not_touch_cnb_shimmed_buildpacks() {
        let toml = r#"
[[buildpacks]]
  id = "heroku/scala"
  uri = "https://cnb-shim.herokuapp.com/v1/heroku/scala?version=0.0.0&name=Scala"

[[order]]
  [[order.group]]
    id = "heroku/scala"
    version = "0.0.0"
"#;
        let mut document = Document::from_str(toml).unwrap();

        update_builder_with_buildpack_info(
            &mut document,
            &buildpack_id!("heroku/scala"),
            &BuildpackVersion::try_from("1.1.1".to_string()).unwrap(),
            "docker://docker.io/heroku/buildpack-scala@sha256:dd41aacd9ce11a11fdc3f0ba0bf4cd8a816fc56c634d30c2806998b5fce9534d",
        )
        .unwrap();

        assert_eq!(
            document.to_string(),
            r#"
[[buildpacks]]
  id = "heroku/scala"
  uri = "https://cnb-shim.herokuapp.com/v1/heroku/scala?version=0.0.0&name=Scala"

[[order]]
  [[order.group]]
    id = "heroku/scala"
    version = "0.0.0"
"#
        );
    }
}
