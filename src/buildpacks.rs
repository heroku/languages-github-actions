use libcnb_common::toml_file::{read_toml_file, TomlFileError};
use libcnb_data::buildpack::BuildpackDescriptor;
use libcnb_package::find_buildpack_dirs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

#[derive(Debug, thiserror::Error)]
pub(crate) enum CalculateDigestError {
    #[error("Failed to execute crane digest {0}\nError: {1}")]
    CommandFailure(String, #[source] std::io::Error),
    #[error("Command crane digest {0} exited with a non-zero status\nStatus: {1}")]
    ExitStatus(String, ExitStatus),
}

pub(crate) fn calculate_digest(digest_url: &str) -> Result<String, CalculateDigestError> {
    let output = Command::new("crane")
        .args(["digest", digest_url])
        .output()
        .map_err(|e| CalculateDigestError::CommandFailure(digest_url.to_owned(), e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(CalculateDigestError::ExitStatus(
            digest_url.to_owned(),
            output.status,
        ))
    }
}

pub(crate) fn read_image_repository_metadata(
    buildpack_descriptor: &BuildpackDescriptor,
) -> Option<String> {
    let metadata = match buildpack_descriptor {
        BuildpackDescriptor::Component(descriptor) => &descriptor.metadata,
        BuildpackDescriptor::Composite(descriptor) => &descriptor.metadata,
    };

    #[allow(clippy::redundant_closure_for_method_calls)]
    metadata
        .as_ref()
        .and_then(|metadata| metadata.get("release").and_then(|value| value.as_table()))
        .and_then(|release| release.get("image").and_then(|value| value.as_table()))
        .and_then(|image| image.get("repository").and_then(|value| value.as_str()))
        .map(|value| value.to_string())
}

pub(crate) fn find_releasable_buildpacks(
    starting_dir: &Path,
) -> Result<Vec<PathBuf>, FindReleasableBuildpacksError> {
    find_buildpack_dirs(starting_dir)
        .map(|results| {
            results
                .into_iter()
                .filter(|dir| dir.join("CHANGELOG.md").exists())
                .collect()
        })
        .map_err(|e| FindReleasableBuildpacksError(starting_dir.to_path_buf(), e))
}
#[derive(Debug, thiserror::Error)]
#[error("I/O error while finding buildpacks\nPath: {}\nError: {1}", .0.display())]
pub(crate) struct FindReleasableBuildpacksError(PathBuf, ignore::Error);

pub(crate) fn read_buildpack_descriptor(
    dir: &Path,
) -> Result<BuildpackDescriptor, ReadBuildpackDescriptorError> {
    let buildpack_path = dir.join("buildpack.toml");
    read_toml_file::<BuildpackDescriptor>(&buildpack_path)
        .map_err(|e| ReadBuildpackDescriptorError(buildpack_path, e))
}

#[derive(Debug, thiserror::Error)]
#[error("Failed to read buildpack descriptor\nPath: {}\nError: {1}", .0.display())]
pub(crate) struct ReadBuildpackDescriptorError(PathBuf, #[source] TomlFileError);

#[cfg(test)]
mod test {
    use crate::buildpacks::read_image_repository_metadata;
    use libcnb_data::buildpack::BuildpackDescriptor;

    #[test]
    fn test_read_image_repository_metadata() {
        let data = r#"
api = "0.9"

[buildpack]
id = "foo/bar"
version = "0.0.1"

[[targets]]
os = "linux"
arch = "amd64"

[[stacks]]
id = "*"

[metadata.release.image]
repository = "repository value"
"#;

        let buildpack_descriptor = toml::from_str::<BuildpackDescriptor>(data).unwrap();
        assert_eq!(
            read_image_repository_metadata(&buildpack_descriptor),
            Some("repository value".to_string())
        );
    }

    #[test]
    fn test_read_image_repository_metadata_empty() {
        let data = r#"
api = "0.9"

[buildpack]
id = "foo/bar"
version = "0.0.1"
"#;

        let buildpack_descriptor = toml::from_str::<BuildpackDescriptor>(data).unwrap();
        assert_eq!(read_image_repository_metadata(&buildpack_descriptor), None);
    }
}
