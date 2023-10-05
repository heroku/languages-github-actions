use std::io;
use std::path::{Path, PathBuf};

pub(crate) mod generate_buildpack_matrix;
pub(crate) mod generate_changelog;
pub(crate) mod prepare_release;
pub(crate) mod update_builder;

pub(crate) fn resolve_working_dir_from_current_dir(value: Option<PathBuf>) -> io::Result<PathBuf> {
    let current_dir_result = std::env::current_dir();
    match value {
        None => current_dir_result,
        Some(path) => current_dir_result.map(|base| resolve_path(&path, &base)),
    }
}

pub(crate) fn resolve_path(path: &Path, base: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}
