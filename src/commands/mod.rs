use std::io;
use std::path::PathBuf;

pub(crate) mod generate_buildpack_matrix;
pub(crate) mod generate_changelog;
pub(crate) mod prepare_release;
pub(crate) mod update_builder;

pub(crate) fn resolve_path(value: Option<PathBuf>) -> Result<PathBuf, ResolvePathError> {
    let current_dir = std::env::current_dir();
    match value {
        None => current_dir,
        Some(path) if path.is_absolute() => Ok(path),
        Some(path) => current_dir.map(|dir| dir.join(path)),
    }
    .map_err(ResolvePathError)
}

#[derive(Debug, thiserror::Error)]
#[error("Failed to get current directory\nError: {0}")]
pub(crate) struct ResolvePathError(io::Error);
