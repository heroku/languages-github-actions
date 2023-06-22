use crate::changelog::ChangelogError;
use crate::github::actions::SetOutputError;
use libcnb_data::buildpack::BuildpackVersion;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::io;
use std::path::PathBuf;

#[derive(Debug)]
pub(crate) enum Error {
    GetCurrentDir(io::Error),
    InvalidRepositoryUrl(String, uriparse::URIError),
    InvalidDeclarationsStartingVersion(String, semver::Error),
    NoBuildpacksFound(PathBuf),
    NotAllVersionsMatch(HashMap<PathBuf, BuildpackVersion>),
    NoFixedVersion,
    FindingBuildpacks(PathBuf, io::Error),
    ReadingChangelog(PathBuf, io::Error),
    ParsingChangelog(PathBuf, ChangelogError),
    ReadingBuildpack(PathBuf, io::Error),
    ParsingBuildpack(PathBuf, toml_edit::TomlError),
    MissingRequiredField(PathBuf, String),
    InvalidBuildpackId(PathBuf, String),
    InvalidBuildpackVersion(PathBuf, String),
    WritingBuildpack(PathBuf, io::Error),
    WritingChangelog(PathBuf, io::Error),
    SetActionOutput(SetOutputError),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::GetCurrentDir(error) => {
                write!(f, "Failed to get current directory\nError: {error}")
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

            Error::FindingBuildpacks(path, error) => {
                write!(
                    f,
                    "I/O error while finding buildpacks\nPath: {}\nError: {error}",
                    path.display()
                )
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

            Error::SetActionOutput(set_output_error) => match set_output_error {
                SetOutputError::Opening(error) | SetOutputError::Writing(error) => {
                    write!(f, "Could not write action output\nError: {error}")
                }
            },

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
