use crate::commands::check_buildpack_registry::errors::Error;
use crate::github;
use clap::Parser;
use libcnb_data::buildpack::{BuildpackId, BuildpackVersion};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

type Result<T> = std::result::Result<T, Error>;

#[derive(Parser, Debug)]
#[command(author, version, about = "Checks the buildpack registry to see if a buildpack is registered", long_about = None)]
pub(crate) struct CheckBuildpackRegistryArgs {
    #[arg(long)]
    pub(crate) buildpack_id: BuildpackId,
    #[arg(long)]
    pub(crate) buildpack_version: String,
    #[arg(long)]
    pub(crate) path: String,
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
struct RegistryEntry {
    ns: String,
    name: String,
    version: String,
    yanked: bool,
    addr: String,
}

pub(crate) fn execute(args: CheckBuildpackRegistryArgs) -> Result<()> {
    let current_dir = std::env::current_dir()
        .map_err(Error::GetCurrentDir)
        .map(|dir| dir.join(PathBuf::from(args.path)))?;

    let buildpack_id = args.buildpack_id;

    let buildpack_version = BuildpackVersion::try_from(args.buildpack_version.to_string())
        .map_err(|e| Error::InvalidBuildpackVersion(args.buildpack_version, e))?;

    let (namespace, name) = get_namespace_and_name(buildpack_id)?;

    let index_path = current_dir.join(get_index_path(&namespace, &name));

    let is_registered = if index_path.exists() {
        let contents = fs::read_to_string(&index_path)
            .map_err(|e| Error::ReadingRegistryIndex(index_path.clone(), e))?;
        let registry_entries = parse_registry_index(&contents)
            .map_err(|e| Error::ParsingRegistryIndex(index_path, e))?;
        registry_entries
            .into_iter()
            .any(|entry| buildpack_version.to_string() == entry.version)
    } else {
        false
    };

    github::actions::set_output("is_registered", is_registered.to_string())
        .map_err(Error::SetOutput)?;

    Ok(())
}

fn get_namespace_and_name(buildpack_id: BuildpackId) -> Result<(String, String)> {
    let parts = buildpack_id.as_str().split('/').collect::<Vec<_>>();
    if let (Some(namespace), Some(name)) = (parts.first(), parts.get(1)) {
        Ok((namespace.to_string(), name.to_string()))
    } else {
        Err(Error::GetNamespaceAndName(buildpack_id))
    }
}

// See https://github.com/buildpacks/github-actions/blob/b3d523fb36b6feee0d6726ed009ae8738f87aa28/registry/internal/index/path.go
fn get_index_path(namespace: &str, name: &str) -> PathBuf {
    let path = match name.len() {
        1 => PathBuf::from("1"),
        2 => PathBuf::from("2"),
        3 => PathBuf::from("3").join(&name[0..2]),
        _ => PathBuf::from(&name[0..2]).join(&name[2..4]),
    };
    path.join(format!("{namespace}_{name}"))
}

fn parse_registry_index(contents: &str) -> serde_json::Result<Vec<RegistryEntry>> {
    contents
        .split('\n')
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str::<RegistryEntry>)
        .collect()
}

#[cfg(test)]
mod test {
    use crate::commands::check_buildpack_registry::command::{
        get_index_path, get_namespace_and_name, parse_registry_index, RegistryEntry,
    };
    use libcnb_data::buildpack_id;
    use std::path::PathBuf;

    #[test]
    fn test_get_namespace_and_name() {
        assert_eq!(
            get_namespace_and_name(buildpack_id!("heroku/jvm")).unwrap(),
            ("heroku".to_string(), "jvm".to_string())
        );
    }

    #[test]
    fn test_get_index_path_when_name_is_len_1() {
        assert_eq!(get_index_path("heroku", "z"), PathBuf::from("1/heroku_z"));
    }

    #[test]
    fn test_get_index_path_when_name_is_len_2() {
        assert_eq!(get_index_path("heroku", "go"), PathBuf::from("2/heroku_go"));
    }

    #[test]
    fn test_get_index_path_when_name_is_len_3() {
        assert_eq!(
            get_index_path("heroku", "php"),
            PathBuf::from("3/ph/heroku_php")
        );
    }

    #[test]
    fn test_get_index_path_when_name_is_greater_than_3() {
        assert_eq!(
            get_index_path("heroku", "scala"),
            PathBuf::from("sc/al/heroku_scala")
        );
    }

    #[test]
    fn test_parse_registry_entry() {
        let contents = r#"
{"ns":"heroku","name":"scala","version":"0.0.88","yanked":true,"addr":"docker.io/heroku/buildpack-scala@sha256:7a58fd9774925a6e23ce638cc4c7b57fdb9a14092632bc00c10081bb226f19b3"}
        "#;

        let registry_entry: RegistryEntry = serde_json::from_str(contents).unwrap();
        assert_eq!(
            registry_entry,
            RegistryEntry {
                ns: "heroku".to_string(),
                name: "scala".to_string(),
                version: "0.0.88".to_string(),
                yanked: true,
                addr: "docker.io/heroku/buildpack-scala@sha256:7a58fd9774925a6e23ce638cc4c7b57fdb9a14092632bc00c10081bb226f19b3".to_string()
            }
        );
    }

    #[test]
    fn test_parse_registry_index() {
        let contents = r#"
{"ns":"heroku","name":"scala","version":"0.0.88","yanked":true,"addr":"docker.io/heroku/buildpack-scala@sha256:7a58fd9774925a6e23ce638cc4c7b57fdb9a14092632bc00c10081bb226f19b3"}
{"ns":"heroku","name":"scala","version":"0.0.89","yanked":false,"addr":"docker.io/heroku/buildpack-scala@sha256:d7381e85924a808bad28d44c988d79e6abb6b5f9dd98ee31996f54a3ea653143"}
{"ns":"heroku","name":"scala","version":"0.0.90","yanked":false,"addr":"docker.io/heroku/buildpack-scala@sha256:39ad34bcc1766cdff4f2a4d5e88931a2f2678f5605bd56f1b6fc551f40fba70f"}
{"ns":"heroku","name":"scala","version":"0.0.91","yanked":false,"addr":"docker.io/heroku/buildpack-scala@sha256:99b35329c3c5af4deaab1892a6ccfad9d0646fcb8bf6fe04c688f0429d95fc11"}
{"ns":"heroku","name":"scala","version":"0.0.92","yanked":false,"addr":"docker.io/heroku/buildpack-scala@sha256:02a26d364465a567782806effa2afbf4b992a1e15453bb8e62c5da27118dc7e6"}
{"ns":"heroku","name":"scala","version":"1.0.0","yanked":false,"addr":"docker.io/heroku/buildpack-scala@sha256:1fc6beaf291dcf1972c621afe3a1b3a80d1bffe8fccb6b0a68bc7f24bf99456a"}
{"ns":"heroku","name":"scala","version":"1.1.1","yanked":false,"addr":"docker.io/heroku/buildpack-scala@sha256:dd41aacd9ce11a11fdc3f0ba0bf4cd8a816fc56c634d30c2806998b5fce9534d"}
        "#;

        let registry_entries = parse_registry_index(contents).unwrap();
        assert_eq!(
            registry_entries
                .into_iter()
                .map(|registry_entry| registry_entry.version)
                .collect::<Vec<_>>(),
            vec!["0.0.88", "0.0.89", "0.0.90", "0.0.91", "0.0.92", "1.0.0", "1.1.1"]
        );
    }
}
