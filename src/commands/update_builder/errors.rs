use crate::buildpack_info::CalculateDigestError;
use crate::commands::update_builder::command::UpdateBuilderError;
use libcnb_package::ReadBuildpackDataError;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;

#[derive(Debug)]
pub(crate) enum Error {
    GetCurrentDir(std::io::Error),
    FindingBuildpacks(PathBuf, std::io::Error),
    ReadingBuildpackData(libcnb_package::ReadBuildpackDataError),
    NoBuildpacks(PathBuf),
    ReadingBuilder(PathBuf, std::io::Error),
    ParsingBuilder(PathBuf, toml_edit::TomlError),
    NoBuilderFiles(Vec<String>),
    UpdatingBuilder(PathBuf, UpdateBuilderError),
    WritingBuilder(PathBuf, std::io::Error),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::GetCurrentDir(error) => {
                write!(f, "Could not get the current directory\nError: {error}")
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

            Error::FindingBuildpacks(path, error) => {
                write!(
                    f,
                    "I/O error while finding buildpacks\nPath: {}\nError: {error}",
                    path.display()
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

            Error::NoBuildpacks(path) => {
                write!(
                    f,
                    "No buildpacks were found in the given directory\nPath: {}",
                    path.display()
                )
            }

            Error::UpdatingBuilder(path, update_error) => match update_error {
                UpdateBuilderError::BuilderMissingRequiredKey(key) => {
                    write!(
                        f,
                        "Missing required key `{key}` in builder\nPath: {}",
                        path.display()
                    )
                }

                UpdateBuilderError::MissingDockerRepositoryMetadata(buildpack_path) => {
                    write!(
                        f,
                        "The following buildpack is missing the metadata.release.docker.repository entry\nPath: {}",
                        buildpack_path.display()
                    )
                }

                UpdateBuilderError::CalculatingDigest(buildpack_path, calculate_digest_error) => {
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
            },
        }
    }
}
