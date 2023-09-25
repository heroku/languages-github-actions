use crate::buildpacks::{FindReleasableBuildpacksError, ReadBuildpackDescriptorError};
use crate::commands::ResolvePathError;
use crate::github::actions::SetActionOutputError;
use std::collections::HashSet;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;

#[derive(Debug)]
pub(crate) enum Error {
    ResolvePath(ResolvePathError),
    FindReleasableBuildpacks(FindReleasableBuildpacksError),
    ReadBuildpackDescriptor(ReadBuildpackDescriptorError),
    MissingDockerRepositoryMetadata(PathBuf),
    SerializingJson(serde_json::Error),
    FixedVersion(HashSet<String>),
    SetActionOutput(SetActionOutputError),
}

impl Display for Error {
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

            Error::ReadBuildpackDescriptor(error) => {
                write!(f, "{error}")
            }

            Error::SerializingJson(error) => {
                write!(
                    f,
                    "Could not serialize buildpacks into json\nError: {error}"
                )
            }

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
