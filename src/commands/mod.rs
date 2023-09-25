use std::fmt::{Display, Formatter};
use std::io;
use std::path::{Path, PathBuf};

pub(crate) mod generate_buildpack_matrix;
pub(crate) mod generate_changelog;
pub(crate) mod prepare_release;
pub(crate) mod update_builder;

pub(crate) fn resolve_path(path: PathBuf, current_dir: &Path) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        current_dir.join(path)
    }
}

pub(crate) fn get_working_directory(
    value: Option<PathBuf>,
) -> Result<PathBuf, GetWorkingDirectoryError> {
    std::env::current_dir()
        .map_err(GetWorkingDirectoryError)
        .map(|current_dir| {
            if let Some(dir) = value {
                resolve_path(dir, &current_dir)
            } else {
                current_dir
            }
        })
}

#[derive(Debug)]
pub(crate) struct GetWorkingDirectoryError(io::Error);

impl Display for GetWorkingDirectoryError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let error = &self.0;
        write!(f, "Failed to get current directory\nError: {error}")
    }
}
