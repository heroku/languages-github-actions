use crate::buildpacks::FindReleasableBuildpacksError;
use crate::changelog::ChangelogError;
use crate::commands::ResolvePathError;
use crate::github::actions::SetActionOutputError;
use libcnb_data::buildpack::BuildpackVersion;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::io;
use std::path::PathBuf;

#[derive(Debug)]
pub(crate) enum Error {
    ResolvePath(ResolvePathError),
    FindReleasableBuildpacks(FindReleasableBuildpacksError),
    SetActionOutput(SetActionOutputError),
    InvalidRepositoryUrl(String, uriparse::URIError),
    InvalidDeclarationsStartingVersion(String, semver::Error),
    NoBuildpacksFound(PathBuf),
    NotAllVersionsMatch(HashMap<PathBuf, BuildpackVersion>),
    NoFixedVersion,
    ReadingChangelog(PathBuf, io::Error),
    ParsingChangelog(PathBuf, ChangelogError),
    ReadingBuildpack(PathBuf, io::Error),
    ParsingBuildpack(PathBuf, toml_edit::TomlError),
    MissingRequiredField(PathBuf, String),
    InvalidBuildpackId(PathBuf, String),
    InvalidBuildpackVersion(PathBuf, String),
    WritingBuildpack(PathBuf, io::Error),
    WritingChangelog(PathBuf, io::Error),
}

impl Display for Error {
    #[allow(clippy::too_many_lines)]
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::ResolvePath(error) => {
                write!(f, "{error}")
            }
            Error::FindReleasableBuildpacks(error) => {
                write!(f, "{error}")
            }
            Error::SetActionOutput(error) => {
                write!(f, "{error}")
            }
            Error::InvalidRepositoryUrl(value, error) => {
                write!(
                    f,
                    "Invalid URL `{value}` for argument --repository-url\nError: {error}"
                )
            }
            Error::InvalidDeclarationsStartingVersion(value, error) => {
                write!(f, "Invalid Version `{value}` for argument --declarations-starting-version\nError: {error}")
            }
            Error::NoBuildpacksFound(path) => {
                write!(f, "No buildpacks found under {}", path.display())
            }
            Error::NotAllVersionsMatch(version_map) => {
                write!(
                    f,
                    "Not all versions match:\n{}",
                    version_map
                        .iter()
                        .map(|(path, version)| format!("• {version} ({})", path.display()))
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            }
            Error::NoFixedVersion => {
                write!(f, "No fixed version could be determined")
            }
            Error::ReadingBuildpack(path, error) => {
                write!(
                    f,
                    "Could not read buildpack\nPath: {}\nError: {error}",
                    path.display()
                )
            }
            Error::ParsingBuildpack(path, error) => {
                write!(
                    f,
                    "Could not parse buildpack\nPath: {}\nError: {error}",
                    path.display()
                )
            }
            Error::WritingBuildpack(path, error) => {
                write!(
                    f,
                    "Could not write buildpack\nPath: {}\nError: {error}",
                    path.display()
                )
            }
            Error::ReadingChangelog(path, error) => {
                write!(
                    f,
                    "Could not read changelog\nPath: {}\nError: {error}",
                    path.display()
                )
            }
            Error::ParsingChangelog(path, error) => {
                write!(
                    f,
                    "Could not parse changelog\nPath: {}\nError: {error}",
                    path.display()
                )
            }
            Error::WritingChangelog(path, error) => {
                write!(
                    f,
                    "Could not write changelog\nPath: {}\nError: {error}",
                    path.display()
                )
            }
            Error::MissingRequiredField(path, field) => {
                write!(
                    f,
                    "Missing required field `{field}` in buildpack.toml\nPath: {}",
                    path.display()
                )
            }
            Error::InvalidBuildpackId(path, id) => {
                write!(
                    f,
                    "Invalid buildpack id `{id}` in buildpack.toml\nPath: {}",
                    path.display()
                )
            }
            Error::InvalidBuildpackVersion(path, version) => {
                write!(
                    f,
                    "Invalid buildpack version `{version}` in buildpack.toml\nPath: {}",
                    path.display()
                )
            }
        }
    }
}
