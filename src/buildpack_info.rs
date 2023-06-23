use libcnb_data::buildpack::{BuildpackDescriptor, BuildpackId, BuildpackVersion};
use libcnb_package::{BuildpackData, GenericMetadata};
use std::path::PathBuf;
use std::process::{Command, ExitStatus};

pub(crate) trait BuildpackInfo {
    fn path(&self) -> PathBuf;
    fn buildpack_id(&self) -> BuildpackId;
    fn buildpack_version(&self) -> BuildpackVersion;
    fn docker_repository(&self) -> Option<String>;
}

impl BuildpackInfo for BuildpackData<GenericMetadata> {
    fn path(&self) -> PathBuf {
        self.buildpack_descriptor_path.clone()
    }

    fn buildpack_id(&self) -> BuildpackId {
        self.buildpack_descriptor.buildpack().id.clone()
    }

    fn buildpack_version(&self) -> BuildpackVersion {
        BuildpackVersion {
            ..self.buildpack_descriptor.buildpack().version
        }
    }

    fn docker_repository(&self) -> Option<String> {
        let metadata = match &self.buildpack_descriptor {
            BuildpackDescriptor::Single(descriptor) => &descriptor.metadata,
            BuildpackDescriptor::Meta(descriptor) => &descriptor.metadata,
        };

        metadata
            .as_ref()
            .and_then(|metadata| metadata.get("release").and_then(|value| value.as_table()))
            .and_then(|release| release.get("docker").and_then(|value| value.as_table()))
            .and_then(|docker| docker.get("repository").and_then(|value| value.as_str()))
            .map(|value| value.to_string())
    }
}

#[derive(Debug)]
pub(crate) enum CalculateDigestError {
    CommandFailure(String, std::io::Error),
    ExitStatus(String, ExitStatus),
}

pub(crate) fn calculate_digest(digest_url: &String) -> Result<String, CalculateDigestError> {
    let output = Command::new("crane")
        .args(["digest", digest_url])
        .output()
        .map_err(|e| CalculateDigestError::CommandFailure(digest_url.clone(), e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(CalculateDigestError::ExitStatus(
            digest_url.clone(),
            output.status,
        ))
    }
}
