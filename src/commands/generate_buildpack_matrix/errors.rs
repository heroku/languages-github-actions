use crate::buildpacks::{FindReleasableBuildpacksError, ReadBuildpackDescriptorError};
use crate::github::actions::SetActionOutputError;
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("Failed to resolve working directory\nError: {0}")]
    ResolveWorkingDir(std::io::Error),
    #[error(transparent)]
    FindReleasableBuildpacks(FindReleasableBuildpacksError),
    #[error(transparent)]
    ReadBuildpackDescriptor(ReadBuildpackDescriptorError),
    #[error("The following buildpack is missing the metadata.release.docker.repository entry\nPath: {}", .0.display())]
    MissingDockerRepositoryMetadata(PathBuf),
    #[error("Could not serialize buildpacks into json\nError: {0}")]
    SerializingJson(#[source] serde_json::Error),
    #[error("Expected all buildpacks to have the same version but multiple versions were found:\n{}", list_versions(.0))]
    FixedVersion(HashSet<String>),
    #[error(transparent)]
    SetActionOutput(SetActionOutputError),
}

fn list_versions(versions: &HashSet<String>) -> String {
    versions
        .iter()
        .map(|version| format!("• {version}"))
        .collect::<Vec<_>>()
        .join("\n")
}
