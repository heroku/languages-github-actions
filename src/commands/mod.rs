use std::path::{Path, PathBuf};

pub(crate) mod generate_buildpack_matrix;
pub(crate) mod generate_changelog;
pub(crate) mod prepare_release;
pub(crate) mod update_builder;

pub(crate) fn resolve_path(path: &Path, base: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}
