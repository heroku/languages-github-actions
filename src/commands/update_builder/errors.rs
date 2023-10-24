use crate::buildpacks::{
    CalculateDigestError, FindReleasableBuildpacksError, ReadBuildpackDescriptorError,
};
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("Failed to resolve path {}\nError: {1}", .0.display())]
    ResolvePath(PathBuf, std::io::Error),
    #[error(transparent)]
    FindReleasableBuildpacks(FindReleasableBuildpacksError),
    #[error(transparent)]
    ReadBuildpackDescriptor(ReadBuildpackDescriptorError),
    #[error("No buildpacks were found in the given directory\nPath: {}", .0.display())]
    NoBuildpacks(PathBuf),
    #[error("Could not read builder\nPath: {}\nError: {1}", .0.display())]
    ReadingBuilder(PathBuf, #[source] std::io::Error),
    #[error("Could not parse builder\nPath: {}\nError: {1}", .0.display())]
    ParsingBuilder(PathBuf, #[source] toml_edit::TomlError),
    #[error("Error writing builder\nPath: {}\nError: {1}", .0.display())]
    WritingBuilder(PathBuf, #[source] std::io::Error),
    #[error("No builder.toml files found in the given builder directories\n{}", list_builders(.0))]
    NoBuilderFiles(Vec<String>),
    #[error("The following buildpack is missing the metadata.release.image.repository entry\nPath: {}", .0.display())]
    MissingImageRepositoryMetadata(PathBuf),
    #[error("Failed to calculate digest for buildpack\nPath: {}\nError: {1}", .0.display())]
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
