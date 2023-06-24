use crate::changelog::ChangelogError;
use crate::github::actions::SetOutputError;
use libcnb_package::ReadBuildpackDataError;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;

#[derive(Debug)]
pub(crate) enum Error {
    GetWorkingDir(std::io::Error),
    FindingBuildpacks(PathBuf, std::io::Error),
    GetBuildpackId(ReadBuildpackDataError),
    ReadingChangelog(PathBuf, std::io::Error),
    ParsingChangelog(PathBuf, ChangelogError),
    SetActionOutput(SetOutputError),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::GetWorkingDir(error) => {
                write!(f, "Failed to get working directory\nError: {error}")
            }

            Error::FindingBuildpacks(path, error) => {
                write!(
                    f,
                    "I/O error while finding buildpacks\nPath: {}\nError: {error}",
                    path.display()
                )
            }

            Error::GetBuildpackId(read_buildpack_data_error) => match read_buildpack_data_error {
                ReadBuildpackDataError::ReadingBuildpack { path, source } => {
                    write!(
                        f,
                        "Error reading buildpack\nPath: {}\nError: {source}",
                        path.display()
                    )
                }

                ReadBuildpackDataError::ParsingBuildpack { path, source } => {
                    write!(
                        f,
                        "Error parsing buildpack\nPath: {}\nError: {source}",
                        path.display()
                    )
                }
            },

            Error::SetActionOutput(set_output_error) => match set_output_error {
                SetOutputError::Opening(error) | SetOutputError::Writing(error) => {
                    write!(f, "Could not write action output\nError: {error}")
                }
            },

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
