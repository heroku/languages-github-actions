use crate::buildpacks::{FindReleasableBuildpacksError, ReadBuildpackDescriptorError};
use crate::changelog::ChangelogError;
use crate::commands::ResolvePathError;
use crate::github::actions::SetActionOutputError;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;

#[derive(Debug)]
pub(crate) enum Error {
    ResolvePath(ResolvePathError),
    FindReleasableBuildpacks(FindReleasableBuildpacksError),
    ReadBuildpackDescriptor(ReadBuildpackDescriptorError),
    ReadingChangelog(PathBuf, std::io::Error),
    ParsingChangelog(PathBuf, ChangelogError),
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

            Error::ReadBuildpackDescriptor(error) => {
                write!(f, "{error}")
            }

            Error::SetActionOutput(error) => {
                write!(f, "{error}")
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
        }
    }
}
