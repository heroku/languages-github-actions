use crate::buildpacks::{
    CalculateDigestError, FindReleasableBuildpacksError, ReadBuildpackDescriptorError,
};
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("Failed to resolve path {0}\nError: {1}")]
    ResolvePath(PathBuf, std::io::Error),
    #[error(transparent)]
    FindReleasableBuildpacks(FindReleasableBuildpacksError),
    #[error(transparent)]
    ReadBuildpackDescriptor(ReadBuildpackDescriptorError),
    #[error("No buildpacks were found in the given directory\nPath: {0}")]
    NoBuildpacks(PathBuf),
    #[error("Could not read builder\nPath: {0}\nError: {1}")]
    ReadingBuilder(PathBuf, #[source] std::io::Error),
    #[error("Could not parse builder\nPath: {0}\nError: {1}")]
    ParsingBuilder(PathBuf, #[source] toml_edit::TomlError),
    #[error("Error writing builder\nPath: {0}\nError: {1}")]
    WritingBuilder(PathBuf, #[source] std::io::Error),
    #[error("No builder.toml files found in the given builder directories\n{}", list_builders(.0))]
    NoBuilderFiles(Vec<String>),
    #[error(
        "The following buildpack is missing the metadata.release.image.repository entry\nPath: {0}"
    )]
    MissingImageRepositoryMetadata(PathBuf),
    #[error("Failed to calculate digest for buildpack\nPath: {0}\nError: {1}")]
    CalculatingDigest(PathBuf, #[source] CalculateDigestError),
    #[error("Missing required key `{0}` in builder")]
    BuilderMissingRequiredKey(String),
}

fn list_builders(builders: &[String]) -> String {
    builders
        .iter()
        .map(|builder| format!("• {builder}"))
        .collect::<Vec<_>>()
        .join("\n")
}
