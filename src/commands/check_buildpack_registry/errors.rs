use crate::github::actions::SetOutputError;
use libcnb_data::buildpack::{BuildpackId, BuildpackVersionError};
use std::fmt::{Display, Formatter};
use std::path::PathBuf;

#[derive(Debug)]
pub(crate) enum Error {
    GetCurrentDir(std::io::Error),
    InvalidBuildpackVersion(String, BuildpackVersionError),
    GetNamespaceAndName(BuildpackId),
    ReadingRegistryIndex(PathBuf, std::io::Error),
    ParsingRegistryIndex(PathBuf, serde_json::Error),
    SetOutput(SetOutputError),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::GetCurrentDir(error) => {
                write!(f, "Could not get the current directory\nError: {error}")
            }

            Error::InvalidBuildpackVersion(version, error) => {
                write!(
                    f,
                    "The buildpack version argument is invalid\nValue: {version}\nError: {error}"
                )
            }

            Error::GetNamespaceAndName(buildpack_id) => {
                write!(
                    f,
                    "The namespace and name could not be determined from the buildpack id\nValue: {buildpack_id}"
                )
            }

            Error::ReadingRegistryIndex(path, error) => {
                write!(
                    f,
                    "Could not read registry index\nPath: {}\nError: {error}",
                    path.display()
                )
            }

            Error::ParsingRegistryIndex(path, error) => {
                write!(
                    f,
                    "Could not parse registry index\nPath: {}\nError: {error}",
                    path.display()
                )
            }

            Error::SetOutput(error) => match error {
                SetOutputError::Opening(error) | SetOutputError::Writing(error) => {
                    write!(f, "Could not write action output\nError: {error}")
                }
            },
        }
    }
}
