use crate::buildpacks::FindReleasableBuildpacksError;
use crate::changelog::ChangelogError;
use crate::github::actions::WriteActionDataError;
use libcnb_data::buildpack::BuildpackVersion;
use std::collections::HashMap;
use std::io;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("Failed to get current directory\nError: {0}")]
    GetCurrentDir(io::Error),
    #[error(transparent)]
    FindReleasableBuildpacks(FindReleasableBuildpacksError),
    #[error(transparent)]
    SetActionOutput(WriteActionDataError),
    #[error("Invalid URL `{0}` for argument --repository-url\nError: {1}")]
    InvalidRepositoryUrl(String, #[source] uriparse::URIError),
    #[error("Invalid Version `{0}` for argument --declarations-starting-version\nError: {1}")]
    InvalidDeclarationsStartingVersion(String, #[source] semver::Error),
    #[error("No buildpacks found under {}", .0.display())]
    NoBuildpacksFound(PathBuf),
    #[error("Not all versions match:\n{}", list_versions_with_path(.0))]
    NotAllVersionsMatch(HashMap<PathBuf, BuildpackVersion>),
    #[error("No fixed version could be determined")]
    NoFixedVersion,
    #[error("Could not read changelog\nPath: {0}\nError: {1}")]
    ReadingChangelog(PathBuf, #[source] io::Error),
    #[error("Could not parse changelog\nPath: {0}\nError: {1}")]
    ParsingChangelog(PathBuf, #[source] ChangelogError),
    #[error("Could not write changelog\nPath: {0}\nError: {1}")]
    WritingChangelog(PathBuf, #[source] io::Error),
    #[error("Missing required field `{1}` in buildpack.toml\nPath: {0}")]
    MissingRequiredField(PathBuf, String),
    #[error("Invalid buildpack id `{1}` in buildpack.toml\nPath: {0}")]
    InvalidBuildpackId(PathBuf, String),
    #[error("Invalid buildpack version `{1}` in buildpack.toml\nPath: {0}")]
    InvalidBuildpackVersion(PathBuf, String),
    #[error("Could not read buildpack\nPath: {0}\nError: {1}")]
    ReadingBuildpack(PathBuf, #[source] io::Error),
    #[error("Could not parse buildpack\nPath: {0}\nError: {1}")]
    ParsingBuildpack(PathBuf, #[source] toml_edit::TomlError),
    #[error("Could not write buildpack\nPath: {0}\nError: {1}")]
    WritingBuildpack(PathBuf, #[source] io::Error),
}

fn list_versions_with_path(version_map: &HashMap<PathBuf, BuildpackVersion>) -> String {
    version_map
        .iter()
        .map(|(path, version)| format!("• {version} ({})", path.display()))
        .collect::<Vec<_>>()
        .join("\n")
}
