use std::fmt::{Display, Formatter};
use std::io;
use std::path::PathBuf;

pub(crate) mod generate_buildpack_matrix;
pub(crate) mod generate_changelog;
pub(crate) mod prepare_release;
pub(crate) mod update_builder;

pub(crate) fn resolve_path(value: Option<PathBuf>) -> Result<PathBuf, ResolvePathError> {
    std::env::current_dir()
        .map_err(ResolvePathError)
        .map(|current_dir| {
            if let Some(path) = value {
                if path.is_absolute() {
                    path
                } else {
                    current_dir.join(path)
                }
            } else {
                current_dir
            }
        })
}

#[derive(Debug)]
pub(crate) struct ResolvePathError(io::Error);

impl Display for ResolvePathError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let error = &self.0;
        write!(f, "Failed to get current directory\nError: {error}")
    }
}
