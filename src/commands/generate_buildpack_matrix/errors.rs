use crate::github::actions::SetOutputError;
use libcnb_package::ReadBuildpackDataError;
use std::collections::HashSet;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;

#[derive(Debug)]
pub(crate) enum Error {
    GetCurrentDir(std::io::Error),
    FindingBuildpacks(PathBuf, std::io::Error),
    ReadingBuildpackData(ReadBuildpackDataError),
    MissingDockerRepositoryMetadata(PathBuf),
    SerializingJson(serde_json::Error),
    FixedVersion(HashSet<String>),
    SetActionOutput(SetOutputError),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::GetCurrentDir(error) => {
                write!(f, "Failed to get current directory\nError: {error}")
            }

            Error::FindingBuildpacks(path, error) => {
                write!(
                    f,
                    "I/O error while finding buildpacks\nPath: {}\nError: {error}",
                    path.display()
                )
            }

            Error::SetActionOutput(set_output_error) => match set_output_error {
                SetOutputError::Opening(error) | SetOutputError::Writing(error) => {
                    write!(f, "Could not write action output\nError: {error}")
                }
            },

            Error::SerializingJson(error) => {
                write!(
                    f,
                    "Could not serialize buildpacks into json\nError: {error}"
                )
            }

            Error::ReadingBuildpackData(error) => match error {
                ReadBuildpackDataError::ReadingBuildpack { path, source } => {
                    write!(
                        f,
                        "Failed to read buildpack\nPath: {}\nError: {source}",
                        path.display()
                    )
                }
                ReadBuildpackDataError::ParsingBuildpack { path, source } => {
                    write!(
                        f,
                        "Failed to parse buildpack\nPath: {}\nError: {source}",
                        path.display()
                    )
                }
            },

            Error::MissingDockerRepositoryMetadata(path) => {
                write!(
                    f,
                    "The following buildpack is missing the metadata.release.docker.repository entry\nPath: {}",
                    path.display()
                )
            }

            Error::FixedVersion(version) => {
                write!(
                    f,
                    "Expected all buildpacks to have the same version but multiple versions were found:\n{}",
                    version
                        .iter()
                        .map(|version| format!("• {version}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            }
        }
    }
}
