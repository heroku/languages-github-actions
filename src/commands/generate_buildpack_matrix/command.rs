use crate::buildpacks::{
    find_releasable_buildpacks, read_buildpack_descriptor, read_image_repository_metadata,
};
use crate::commands::generate_buildpack_matrix::errors::Error;
use crate::commands::resolve_path;
use crate::github::actions;
use clap::Parser;
use libcnb_data::buildpack::{BuildpackDescriptor, BuildpackId, BuildpackTarget};
use libcnb_data::generic::GenericMetadata;
use libcnb_package::output::{
    create_packaged_buildpack_dir_resolver, default_buildpack_directory_name,
};
use libcnb_package::CargoProfile;
use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

type Result<T> = std::result::Result<T, Error>;

#[derive(Parser, Debug)]
#[command(author, version, about = "Generates a JSON list of buildpack information for each buildpack detected", long_about = None)]
pub(crate) struct GenerateBuildpackMatrixArgs {
    #[arg(long)]
    pub(crate) source_dir: Option<PathBuf>,
    #[arg(long)]
    pub(crate) package_dir: Option<PathBuf>,
    #[arg(long)]
    pub(crate) temporary_id: String,
}

pub(crate) fn execute(args: &GenerateBuildpackMatrixArgs) -> Result<()> {
    let source_dir = match &args.source_dir {
        Some(path) => path.clone(),
        None => std::env::current_dir().map_err(Error::GetCurrentDir)?,
    };
    let package_dir = resolve_path(
        match &args.package_dir {
            Some(path) => path,
            None => Path::new("./packaged"),
        },
        &source_dir,
    );

    let buildpack_dirs =
        find_releasable_buildpacks(&source_dir).map_err(Error::FindReleasableBuildpacks)?;

    let buildpacks = buildpack_dirs
        .iter()
        .map(|dir| read_buildpack_descriptor(dir).map_err(Error::ReadBuildpackDescriptor))
        .collect::<Result<Vec<_>>>()?;

    let buildpacks_info = buildpack_dirs
        .iter()
        .zip(buildpacks.iter())
        .map(|(buildpack_dir, buildpack_descriptor)| {
            read_buildpack_info(
                buildpack_descriptor,
                buildpack_dir,
                &package_dir,
                &args.temporary_id,
            )
        })
        .collect::<Result<Vec<_>>>()?;

    let buildpacks_json =
        serde_json::to_string_pretty(&buildpacks_info).map_err(Error::SerializingJson)?;

    actions::set_output("buildpacks", &buildpacks_json).map_err(Error::WriteActionData)?;
    actions::set_summary(format!(
        "<details><summary>Buildpack Matrix</summary>\n\n```json\n{buildpacks_json}\n```\n</details>"
    ))
    .map_err(Error::WriteActionData)?;

    let versions = buildpacks
        .iter()
        .map(|buildpack_descriptor| buildpack_descriptor.buildpack().version.to_string())
        .collect::<HashSet<_>>();

    if versions.len() != 1 {
        Err(Error::FixedVersion(versions.clone()))?;
    }

    let version = versions
        .iter()
        .next()
        .ok_or(Error::FixedVersion(versions.clone()))?;

    actions::set_output("version", version).map_err(Error::WriteActionData)?;

    let rust_triples = buildpacks
        .iter()
        .flat_map(read_buildpack_targets)
        .filter_map(|t| rust_triple(&t).ok())
        .collect::<HashSet<String>>();

    actions::set_output(
        "rust_triples",
        serde_json::to_string(&rust_triples).map_err(Error::SerializingJson)?,
    )
    .map_err(Error::WriteActionData)?;

    Ok(())
}

#[derive(Serialize)]
pub(crate) struct BuildpackInfo {
    buildpack_id: String,
    buildpack_version: String,
    buildpack_type: BuildpackType,
    buildpack_dir: PathBuf,
    targets: Vec<TargetInfo>,
    image_repository: String,
    stable_tag: String,
    temporary_tag: String,
}

#[derive(Serialize)]
pub(crate) struct TargetInfo {
    os: Option<String>,
    arch: Option<String>,
    rust_triple: Option<String>,
    cnb_file: String,
    stable_tag: String,
    temporary_tag: String,
    output_dir: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum BuildpackType {
    Bash,
    Composite,
    Libcnb,
}

pub(crate) fn read_buildpack_info(
    buildpack_descriptor: &BuildpackDescriptor,
    buildpack_dir: &Path,
    package_dir: &Path,
    temporary_id: &str,
) -> Result<BuildpackInfo> {
    let version = buildpack_descriptor.buildpack().version.to_string();
    let image_repository = read_image_repository_metadata(buildpack_descriptor).ok_or(
        Error::MissingImageRepositoryMetadata(buildpack_dir.join("buildpack.toml")),
    )?;
    let targets = read_buildpack_targets(buildpack_descriptor);
    let buildpack_type = buildpack_type(buildpack_descriptor, buildpack_dir)?;
    Ok(BuildpackInfo {
        buildpack_id: buildpack_descriptor.buildpack().id.to_string(),
        buildpack_version: version.clone(),
        buildpack_dir: buildpack_dir.into(),
        buildpack_type: buildpack_type.clone(),
        targets: read_buildpack_targets(buildpack_descriptor)
            .iter()
            .map(|target| {
                let suffix = if targets.len() > 1 {
                    Some(target_name(target))
                } else {
                    None
                };
                Ok(TargetInfo {
                    cnb_file: cnb_file(&buildpack_descriptor.buildpack().id, suffix.as_deref()),
                    os: target.os.clone(),
                    arch: target.arch.clone(),
                    output_dir: target_output_dir(
                        &buildpack_descriptor.buildpack().id,
                        &buildpack_type,
                        package_dir,
                        target,
                    )?,
                    rust_triple: rust_triple(target).ok(),
                    stable_tag: generate_tag(&image_repository, &version, suffix.as_deref()),
                    temporary_tag: generate_tag(
                        &image_repository,
                        &format!("_{temporary_id}"),
                        suffix.as_deref(),
                    ),
                })
            })
            .collect::<Result<Vec<_>>>()?,
        stable_tag: generate_tag(&image_repository, &version, None),
        temporary_tag: generate_tag(&image_repository, &format!("_{temporary_id}"), None),
        image_repository,
    })
}

// Reads targets from buildpacks while ensuring each buildpack returns at least
// one target (libcnb assumes a linux/amd64 target by default, even if no
// targets are defined).
fn read_buildpack_targets(buildpack_descriptor: &BuildpackDescriptor) -> Vec<BuildpackTarget> {
    let mut targets = match buildpack_descriptor {
        BuildpackDescriptor::Component(descriptor) => descriptor.targets.clone(),
        BuildpackDescriptor::Composite(descriptor) => {
            read_metadata_targets(descriptor.metadata.clone()).unwrap_or_default()
        }
    };
    if targets.is_empty() {
        targets.push(BuildpackTarget {
            os: Some("linux".into()),
            arch: Some("amd64".into()),
            variant: None,
            distros: vec![],
        });
    };
    targets
}

fn generate_tag(repo: &str, tag: &str, suffix: Option<&str>) -> String {
    suffix.map_or_else(
        || format!("{repo}:{tag}"),
        |suffix| format!("{repo}:{tag}_{suffix}"),
    )
}

fn cnb_file(buildpack_id: &BuildpackId, suffix: Option<&str>) -> String {
    let name = default_buildpack_directory_name(buildpack_id);
    suffix.map_or_else(
        || format!("{name}.cnb"),
        |suffix| format!("{name}_{suffix}.cnb"),
    )
}

// Returns the target naming suffix for image tags and .cnb files.
fn target_name(target: &BuildpackTarget) -> String {
    match (target.os.as_deref(), target.arch.as_deref()) {
        (Some(os), Some(arch)) => format!("{os}-{arch}"),
        (Some(os), None) => os.to_string(),
        (None, Some(arch)) => format!("universal-{arch}"),
        (_, _) => "universal".to_string(),
    }
}

fn rust_triple(target: &BuildpackTarget) -> Result<String> {
    match (target.os.as_deref(), target.arch.as_deref()) {
        (Some("linux"), Some("amd64")) => Ok(String::from("x86_64-unknown-linux-musl")),
        (Some("linux"), Some("arm64")) => Ok(String::from("aarch64-unknown-linux-musl")),
        (_, _) => Err(Error::UnknownRustTarget(target.clone())),
    }
}

// Returns the expected output directory for a target. libcnb.rs and composite
// buildpacks should return the libcnb.rs packaged directory.
// (e.g.: packaged/x86_64-unknown-linux-musl/release/heroku_procfile),
// while bash buildpacks should return a similar path, without relying on a
// rust triple.
fn target_output_dir(
    buildpack_id: &BuildpackId,
    buildpack_type: &BuildpackType,
    package_dir: &Path,
    target: &BuildpackTarget,
) -> Result<PathBuf> {
    let target_dirname = match buildpack_type {
        BuildpackType::Bash => target_name(target),
        _ => rust_triple(target)?,
    };
    Ok(create_packaged_buildpack_dir_resolver(
        package_dir,
        CargoProfile::Release,
        &target_dirname,
    )(buildpack_id))
}

fn buildpack_type(
    buildpack_descriptor: &BuildpackDescriptor,
    buildpack_dir: &Path,
) -> Result<BuildpackType> {
    match (
        buildpack_descriptor,
        has_cargo_toml(buildpack_dir),
        has_bin_files(buildpack_dir),
    ) {
        (BuildpackDescriptor::Composite(_), false, false) => Ok(BuildpackType::Composite),
        (BuildpackDescriptor::Composite(_), _, _) => {
            Err(Error::MultipleTypes(buildpack_dir.into()))
        }
        (BuildpackDescriptor::Component(_), true, false) => Ok(BuildpackType::Libcnb),
        (BuildpackDescriptor::Component(_), false, true) => Ok(BuildpackType::Bash),
        (BuildpackDescriptor::Component(_), false, false) => {
            Err(Error::UnknownType(buildpack_dir.into()))
        }
        (_, true, true) => Err(Error::MultipleTypes(buildpack_dir.into())),
    }
}

fn has_cargo_toml(buildpack_dir: &Path) -> bool {
    buildpack_dir.join("Cargo.toml").exists()
}

fn has_bin_files(buildpack_dir: &Path) -> bool {
    ["detect", "build"]
        .iter()
        .all(|file| buildpack_dir.join("bin").join(file).exists())
}

// Project descriptors for composite buildpacks don't support `[[targets]]`,
// but this project needs a way to determine what targets to package composite
// buildpacks for. This function reads `[[targets]]` out of a project
// descriptor's metadata (which is unrestricted) instead.
fn read_metadata_targets(md: GenericMetadata) -> Option<Vec<BuildpackTarget>> {
    let get_toml_string = |table: &toml::Table, key: &str| -> Option<String> {
        Some(table.get(key)?.as_str()?.to_string())
    };
    Some(
        md?.get("targets")?
            .as_array()?
            .iter()
            .filter_map(|tgt_value| {
                let tgt_table = tgt_value.as_table()?;
                Some(BuildpackTarget {
                    os: get_toml_string(tgt_table, "os"),
                    arch: get_toml_string(tgt_table, "arch"),
                    variant: get_toml_string(tgt_table, "variant"),
                    distros: vec![],
                })
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::read_buildpack_info;
    use crate::commands::generate_buildpack_matrix::command::BuildpackType;
    use libcnb_data::buildpack::BuildpackDescriptor;
    use std::{
        fs::{create_dir_all, OpenOptions},
        path::PathBuf,
    };
    use tempfile::tempdir;

    #[test]
    fn read_multitarget_libcnb_buildpack() {
        let bp_descriptor: BuildpackDescriptor = toml::from_str(
            r#"
                api = "0.10"
                [buildpack]
                id = "heroku/fakeymcfakeface"
                version = "1.2.3"
                [[targets]]
                os="linux"
                arch="amd64"
                [[targets]]
                os="linux"
                arch="arm64"
                [metadata.release]
                image = { repository = "docker.io/heroku/buildpack-fakey" }
            "#,
        )
        .expect("expected buildpack descriptor to parse");
        let package_dir = PathBuf::from("./packaged-fake");
        let bp_dir = tempdir().expect("Error creating tempdir");
        OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(bp_dir.path().join("Cargo.toml"))
            .expect("Couldn't write dummy Cargo.toml");

        let bp_info = read_buildpack_info(&bp_descriptor, bp_dir.path(), &package_dir, "918273")
            .expect("Expected to read buildpack info");
        assert_eq!(bp_info.buildpack_id, "heroku/fakeymcfakeface");
        assert_eq!(bp_info.buildpack_type, BuildpackType::Libcnb);
        assert_eq!(
            bp_info.temporary_tag,
            "docker.io/heroku/buildpack-fakey:_918273"
        );
        assert_eq!(bp_info.targets[0].os, Some("linux".to_string()));
        assert_eq!(bp_info.targets[1].arch, Some("arm64".to_string()));
        assert_eq!(
            bp_info.targets[0].rust_triple,
            Some("x86_64-unknown-linux-musl".to_string())
        );
        assert_eq!(
            bp_info.targets[1].rust_triple,
            Some("aarch64-unknown-linux-musl".to_string())
        );
        assert_eq!(
            bp_info.targets[0].temporary_tag,
            "docker.io/heroku/buildpack-fakey:_918273_linux-amd64"
        );
        assert_eq!(
            bp_info.targets[1].stable_tag,
            "docker.io/heroku/buildpack-fakey:1.2.3_linux-arm64"
        );
        assert_eq!(
            bp_info.targets[0].output_dir,
            PathBuf::from(
                "./packaged-fake/x86_64-unknown-linux-musl/release/heroku_fakeymcfakeface"
            )
        );
    }

    #[test]
    fn read_targetless_bash_buildpack() {
        let bp_descriptor: BuildpackDescriptor = toml::from_str(
            r#"
                api = "0.10"
                [buildpack]
                id = "heroku/fakeymcfakeface"
                version = "3.2.1"
                [[stacks]]
                id = "*"
                [metadata.release]
                image = { repository = "docker.io/heroku/buildpack-fakey" }
            "#,
        )
        .expect("expected buildpack descriptor to parse");
        let package_dir = PathBuf::from("./packaged-fake");
        let bp_dir = tempdir().expect("Error creating tempdir");
        create_dir_all(bp_dir.path().join("bin")).expect("Couldn't create bash bin directory");
        for filename in ["detect", "build"] {
            OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(bp_dir.path().join("bin").join(filename))
                .expect("Couldn't write dummy bash file");
        }

        let bp_info = read_buildpack_info(&bp_descriptor, bp_dir.path(), &package_dir, "1928273")
            .expect("Expected to read buildpack info");

        assert_eq!(bp_info.buildpack_id, "heroku/fakeymcfakeface");
        assert_eq!(bp_info.buildpack_type, BuildpackType::Bash);
        assert_eq!(bp_info.stable_tag, "docker.io/heroku/buildpack-fakey:3.2.1");
        assert_eq!(
            bp_info.targets[0].temporary_tag,
            "docker.io/heroku/buildpack-fakey:_1928273"
        );
        assert_eq!(bp_info.targets[0].os, Some("linux".to_string()));
        assert_eq!(bp_info.targets[0].arch, Some("amd64".to_string()));
        assert_eq!(
            bp_info.targets[0].output_dir,
            PathBuf::from("./packaged-fake/linux-amd64/release/heroku_fakeymcfakeface")
        );
    }

    #[test]
    fn read_composite_buildpack() {
        let bp_descriptor: BuildpackDescriptor = toml::from_str(
            r#"
                api = "0.10"
                [buildpack]
                id = "heroku/fakeymcfakeface"
                version = "3.2.1"
                [[order]]
                [[order.group]]
                id = "heroku/nodejs-engine"
                version = "3.0.5"
                [[metadata.targets]]
                os = "linux"
                arch = "amd64"
                [[metadata.targets]]
                os = "linux"
                arch = "arm64"
                [metadata.release]
                image = { repository = "docker.io/heroku/buildpack-fakey" }
            "#,
        )
        .expect("expected buildpack descriptor to parse");
        let package_dir = PathBuf::from("./packaged-fake");
        let bp_dir = tempdir().expect("Error creating tempdir");

        let bp_info = read_buildpack_info(&bp_descriptor, bp_dir.path(), &package_dir, "1928273")
            .expect("Expected to read buildpack info");

        assert_eq!(bp_info.buildpack_id, "heroku/fakeymcfakeface");
        assert_eq!(bp_info.buildpack_type, BuildpackType::Composite);

        assert_eq!(bp_info.targets[0].os, Some("linux".to_string()));
        assert_eq!(bp_info.targets[0].arch, Some("amd64".to_string()));
        assert_eq!(bp_info.targets[1].arch, Some("arm64".to_string()));
        assert_eq!(
            bp_info.targets[1].output_dir,
            PathBuf::from(
                "./packaged-fake/aarch64-unknown-linux-musl/release/heroku_fakeymcfakeface"
            )
        );
    }
}
