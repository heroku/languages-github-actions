use crate::buildpacks::{FindReleasableBuildpacksError, ReadBuildpackDescriptorError};
use crate::changelog::ChangelogError;
use crate::github::actions::WriteActionDataError;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("Failed to get current directory\nError: {0}")]
    GetCurrentDir(std::io::Error),
    #[error(transparent)]
    FindReleasableBuildpacks(FindReleasableBuildpacksError),
    #[error(transparent)]
    ReadBuildpackDescriptor(ReadBuildpackDescriptorError),
    #[error("Could not read changelog\nPath: {0}\nError: {1}")]
    ReadingChangelog(PathBuf, #[source] std::io::Error),
    #[error("Could not parse changelog\nPath: {0}\nError: {1}")]
    ParsingChangelog(PathBuf, #[source] ChangelogError),
    #[error(transparent)]
    SetActionOutput(WriteActionDataError),
}
