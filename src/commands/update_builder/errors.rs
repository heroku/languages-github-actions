use crate::buildpacks::{
    CalculateDigestError, FindReleasableBuildpacksError, ReadBuildpackDescriptorError,
};
use crate::commands::ResolvePathError;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;

#[derive(Debug)]
pub(crate) enum Error {
    ResolvePath(ResolvePathError),
    FindReleasableBuildpacks(FindReleasableBuildpacksError),
    ReadBuildpackDescriptor(ReadBuildpackDescriptorError),
    NoBuildpacks(PathBuf),
    ReadingBuilder(PathBuf, std::io::Error),
    ParsingBuilder(PathBuf, toml_edit::TomlError),
    NoBuilderFiles(Vec<String>),
    MissingDockerRepositoryMetadata(PathBuf),
    CalculatingDigest(PathBuf, CalculateDigestError),
    BuilderMissingRequiredKey(String),
    WritingBuilder(PathBuf, std::io::Error),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::ResolvePath(error) => {
                write!(f, "{error}")
            }

            Error::ReadingBuilder(path, error) => {
                write!(
                    f,
                    "Could not read builder\nPath: {}\nError: {error}",
                    path.display()
                )
            }

            Error::ParsingBuilder(path, error) => {
                write!(
                    f,
                    "Could not parse builder\nPath: {}\nError: {error}",
                    path.display()
                )
            }

            Error::WritingBuilder(path, error) => {
                write!(
                    f,
                    "Error writing builder\nPath: {}\nError: {error}",
                    path.display()
                )
            }

            Error::NoBuilderFiles(builders) => {
                write!(
                    f,
                    "No builder.toml files found in the given builder directories\n{}",
                    builders
                        .iter()
                        .map(|builder| format!("• {builder}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            }

            Error::FindReleasableBuildpacks(error) => {
                write!(f, "{error}")
            }

            Error::ReadBuildpackDescriptor(error) => {
                write!(f, "{error}")
            }

            Error::NoBuildpacks(path) => {
                write!(
                    f,
                    "No buildpacks were found in the given directory\nPath: {}",
                    path.display()
                )
            }

            Error::BuilderMissingRequiredKey(key) => {
                write!(f, "Missing required key `{key}` in builder",)
            }

            Error::MissingDockerRepositoryMetadata(buildpack_path) => {
                write!(
                    f,
                    "The following buildpack is missing the metadata.release.docker.repository entry\nPath: {}",
                    buildpack_path.display()
                )
            }

            Error::CalculatingDigest(buildpack_path, calculate_digest_error) => {
                match calculate_digest_error {
                    CalculateDigestError::CommandFailure(digest_url, error) => {
                        write!(
                            f,
                            "Failed to execute crane digest {digest_url}\nPath: {}\nError: {error}",
                            buildpack_path.display()
                        )
                    }

                    CalculateDigestError::ExitStatus(digest_url, status) => {
                        write!(
                            f,
                            "Command crane digest {digest_url} exited with a non-zero status\nStatus: {status}",
                        )
                    }
                }
            }
        }
    }
}
