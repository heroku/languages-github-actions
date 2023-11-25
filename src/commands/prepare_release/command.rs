use crate::buildpacks::find_releasable_buildpacks;
use crate::commands::prepare_release::errors::Error;
use crate::github::actions;
use clap::{Parser, ValueEnum};
use keep_a_changelog::{ChangeGroup, Changelog, PromoteOptions, ReleaseLink, ReleaseTag};
use libcnb_data::buildpack::{BuildpackId, BuildpackVersion};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs::write;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use toml_edit::{value, ArrayOfTables, Document, Table};

type Result<T> = std::result::Result<T, Error>;

#[derive(Parser, Debug)]
#[command(author, version, about = "Bumps the version of each detected buildpack and adds an entry for any unreleased changes from the changelog", long_about = None)]
pub(crate) struct PrepareReleaseArgs {
    #[arg(long, value_enum)]
    pub(crate) bump: BumpCoordinate,
    #[arg(long)]
    pub(crate) repository_url: String,
}

#[derive(ValueEnum, Debug, Clone)]
pub(crate) enum BumpCoordinate {
    Major,
    Minor,
    Patch,
}

struct BuildpackFile {
    path: PathBuf,
    document: Document,
}

struct ChangelogFile {
    path: PathBuf,
    changelog: Changelog,
}

pub(crate) fn execute(args: PrepareReleaseArgs) -> Result<()> {
    let current_dir = std::env::current_dir().map_err(Error::GetCurrentDir)?;

    let repository_url = args.repository_url;

    let buildpack_dirs =
        find_releasable_buildpacks(&current_dir).map_err(Error::FindReleasableBuildpacks)?;

    if buildpack_dirs.is_empty() {
        Err(Error::NoBuildpacksFound(current_dir))?;
    }

    let buildpack_files = buildpack_dirs
        .iter()
        .map(|dir| read_buildpack_file(dir.join("buildpack.toml")))
        .collect::<Result<Vec<_>>>()?;

    let changelog_files = buildpack_dirs
        .iter()
        .map(|dir| read_changelog_file(dir.join("CHANGELOG.md")))
        .collect::<Result<Vec<_>>>()?;

    let updated_buildpack_ids = buildpack_files
        .iter()
        .map(get_buildpack_id)
        .collect::<Result<HashSet<_>>>()?;

    let current_version = get_fixed_version(&buildpack_files)?;

    let next_version = get_next_version(&current_version, &args.bump);

    for (mut buildpack_file, mut changelog_file) in buildpack_files.into_iter().zip(changelog_files)
    {
        let updated_dependencies = get_buildpack_dependency_ids(&buildpack_file)?
            .into_iter()
            .filter(|buildpack_id| updated_buildpack_ids.contains(buildpack_id))
            .collect::<HashSet<_>>();

        let new_buildpack_contents = update_buildpack_contents_with_new_version(
            &mut buildpack_file,
            &next_version,
            &updated_dependencies,
        )?;

        write(&buildpack_file.path, new_buildpack_contents)
            .map_err(|e| Error::WritingBuildpack(buildpack_file.path.clone(), e))?;

        eprintln!(
            "✅️ Updated version {current_version} → {next_version}: {}",
            buildpack_file.path.display(),
        );

        promote_changelog_unreleased_to_version(
            &mut changelog_file.changelog,
            &next_version,
            &repository_url,
            &updated_dependencies,
        )?;

        write(&changelog_file.path, changelog_file.changelog.to_string())
            .map_err(|e| Error::WritingChangelog(changelog_file.path.clone(), e))?;

        eprintln!(
            "✅️ Added release entry {next_version}: {}",
            changelog_file.path.display()
        );
    }

    actions::set_output("from_version", current_version.to_string())
        .map_err(Error::SetActionOutput)?;
    actions::set_output("to_version", next_version.to_string()).map_err(Error::SetActionOutput)?;

    Ok(())
}

fn read_buildpack_file(path: PathBuf) -> Result<BuildpackFile> {
    let contents =
        std::fs::read_to_string(&path).map_err(|e| Error::ReadingBuildpack(path.clone(), e))?;
    let document =
        Document::from_str(&contents).map_err(|e| Error::ParsingBuildpack(path.clone(), e))?;
    Ok(BuildpackFile { path, document })
}

fn read_changelog_file(path: PathBuf) -> Result<ChangelogFile> {
    let contents =
        std::fs::read_to_string(&path).map_err(|e| Error::ReadingChangelog(path.clone(), e))?;
    let changelog = contents
        .parse()
        .map_err(|e| Error::ParsingChangelog(path.clone(), e))?;
    Ok(ChangelogFile { path, changelog })
}

fn get_buildpack_id(buildpack_file: &BuildpackFile) -> Result<BuildpackId> {
    let buildpack_id = buildpack_file
        .document
        .get("buildpack")
        .and_then(toml_edit::Item::as_table_like)
        .and_then(|buildpack| buildpack.get("id"))
        .and_then(|id| id.as_str().map(std::string::ToString::to_string))
        .ok_or(Error::MissingRequiredField(
            buildpack_file.path.clone(),
            "buildpack.id".to_string(),
        ))?;
    buildpack_id
        .parse()
        .map_err(|_| Error::InvalidBuildpackId(buildpack_file.path.clone(), buildpack_id.clone()))
}

fn get_buildpack_version(buildpack_file: &BuildpackFile) -> Result<BuildpackVersion> {
    let version = buildpack_file
        .document
        .get("buildpack")
        .and_then(toml_edit::Item::as_table_like)
        .and_then(|buildpack| buildpack.get("version"))
        .and_then(|version| version.as_str().map(std::string::ToString::to_string))
        .ok_or(Error::MissingRequiredField(
            buildpack_file.path.clone(),
            "buildpack.version".to_string(),
        ))?;
    BuildpackVersion::try_from(version.clone())
        .map_err(|_| Error::InvalidBuildpackVersion(buildpack_file.path.clone(), version))
}

fn get_buildpack_dependency_ids(buildpack_file: &BuildpackFile) -> Result<HashSet<BuildpackId>> {
    buildpack_file
        .document
        .get("order")
        .and_then(toml_edit::Item::as_array_of_tables)
        .unwrap_or(&ArrayOfTables::default())
        .iter()
        .flat_map(|order| {
            order
                .get("group")
                .and_then(toml_edit::Item::as_array_of_tables)
                .unwrap_or(&ArrayOfTables::default())
                .iter()
                .map(|group| get_group_buildpack_id(group, &buildpack_file.path))
                .collect::<Vec<_>>()
        })
        .collect::<Result<HashSet<_>>>()
}

fn get_group_buildpack_id(group: &Table, path: &Path) -> Result<BuildpackId> {
    group
        .get("id")
        .and_then(toml_edit::Item::as_str)
        .ok_or(Error::MissingRequiredField(
            path.to_path_buf(),
            "order[].group[].id".to_string(),
        ))
        .and_then(|id| {
            id.parse::<BuildpackId>()
                .map_err(|_| Error::InvalidBuildpackId(path.to_path_buf(), id.to_string()))
        })
}

fn get_fixed_version(buildpack_files: &[BuildpackFile]) -> Result<BuildpackVersion> {
    let version_map = buildpack_files
        .iter()
        .map(|buildpack_file| {
            get_buildpack_version(buildpack_file)
                .map(|version| (buildpack_file.path.clone(), version))
        })
        .collect::<Result<HashMap<_, _>>>()?;

    let versions = version_map
        .values()
        .map(std::string::ToString::to_string)
        .collect::<HashSet<_>>();

    if versions.len() != 1 {
        return Err(Error::NotAllVersionsMatch(version_map));
    }

    version_map
        .into_iter()
        .next()
        .map(|(_, version)| version)
        .ok_or(Error::NoFixedVersion)
}

fn get_next_version(current_version: &BuildpackVersion, bump: &BumpCoordinate) -> BuildpackVersion {
    let BuildpackVersion {
        major,
        minor,
        patch,
    } = current_version;

    match bump {
        BumpCoordinate::Major => BuildpackVersion {
            major: major + 1,
            minor: 0,
            patch: 0,
        },
        BumpCoordinate::Minor => BuildpackVersion {
            major: *major,
            minor: minor + 1,
            patch: 0,
        },
        BumpCoordinate::Patch => BuildpackVersion {
            major: *major,
            minor: *minor,
            patch: patch + 1,
        },
    }
}

fn update_buildpack_contents_with_new_version(
    buildpack_file: &mut BuildpackFile,
    next_version: &BuildpackVersion,
    updated_dependencies: &HashSet<BuildpackId>,
) -> Result<String> {
    let buildpack = buildpack_file
        .document
        .get_mut("buildpack")
        .and_then(toml_edit::Item::as_table_like_mut)
        .ok_or(Error::MissingRequiredField(
            buildpack_file.path.clone(),
            "buildpack".to_string(),
        ))?;

    buildpack.insert("version", value(next_version.to_string()));

    let mut empty_orders = ArrayOfTables::default();
    let mut empty_groups = ArrayOfTables::default();

    let orders = buildpack_file
        .document
        .get_mut("order")
        .and_then(toml_edit::Item::as_array_of_tables_mut)
        .unwrap_or(&mut empty_orders);
    for order in orders.iter_mut() {
        let groups = order
            .get_mut("group")
            .and_then(toml_edit::Item::as_array_of_tables_mut)
            .unwrap_or(&mut empty_groups);
        for group in groups.iter_mut() {
            let buildpack_id = get_group_buildpack_id(group, &buildpack_file.path)?;
            if updated_dependencies.contains(&buildpack_id) {
                group.insert("version", value(next_version.to_string()));
            }
        }
    }

    Ok(buildpack_file.document.to_string())
}

fn promote_changelog_unreleased_to_version(
    changelog: &mut Changelog,
    next_version: &BuildpackVersion,
    repository_url: &String,
    updated_dependencies: &HashSet<BuildpackId>,
) -> Result<()> {
    // record dependency updates in the changelog
    let sorted_updated_dependencies = updated_dependencies
        .iter()
        .map(ToString::to_string)
        .collect::<BTreeSet<_>>();
    for updated_dependency in sorted_updated_dependencies {
        changelog.unreleased.add(
            ChangeGroup::Changed,
            format!("Updated `{updated_dependency}` to `{next_version}`."),
        );
    }

    // create a new release entry from unreleased
    let release_version: keep_a_changelog::Version = next_version
        .to_string()
        .parse()
        .map_err(Error::ParseChangelogReleaseVersion)?;

    let previous_version = changelog
        .releases
        .into_iter()
        .next()
        .map(|release| release.version.clone());

    let new_release_link: ReleaseLink = if let Some(value) = previous_version {
        format!("{repository_url}/compare/v{value}...v{release_version}")
    } else {
        format!("{repository_url}/releases/tag/v{release_version}")
    }
    .parse()
    .map_err(Error::ParseReleaseLink)?;

    let mut promote_options =
        PromoteOptions::new(release_version.clone()).with_link(new_release_link);
    if changelog.unreleased.changes.is_empty() {
        promote_options = promote_options.with_tag(ReleaseTag::NoChanges);
    }

    changelog
        .promote_unreleased(&promote_options)
        .map_err(Error::PromoteUnreleased)?;

    changelog.unreleased.link = Some(
        format!("{repository_url}/compare/v{release_version}...HEAD")
            .parse()
            .map_err(Error::ParseReleaseLink)?,
    );

    Ok(())
}

#[cfg(test)]
mod test {
    use crate::commands::prepare_release::command::{
        get_fixed_version, promote_changelog_unreleased_to_version,
        update_buildpack_contents_with_new_version, BuildpackFile,
    };
    use crate::commands::prepare_release::errors::Error;
    use keep_a_changelog::{Changelog, ReleaseDate};
    use libcnb_data::buildpack::BuildpackVersion;
    use libcnb_data::buildpack_id;
    use std::collections::{HashMap, HashSet};
    use std::path::PathBuf;
    use std::str::FromStr;
    use toml_edit::Document;

    #[test]
    fn test_get_fixed_version() {
        let buildpack_a = create_buildpack_file_with_name(
            "/a/buildpack.toml",
            r#"[buildpack]
id = "a"
version = "0.0.0"
"#,
        );
        let buildpack_b = create_buildpack_file_with_name(
            "/b/buildpack.toml",
            r#"[buildpack]
id = "b"
version = "0.0.0"
"#,
        );
        assert_eq!(
            get_fixed_version(&vec![buildpack_a, buildpack_b]).unwrap(),
            BuildpackVersion {
                major: 0,
                minor: 0,
                patch: 0
            }
        );
    }

    #[test]
    fn test_get_fixed_version_errors_if_there_is_a_version_mismatch() {
        let buildpack_a = create_buildpack_file_with_name(
            "/a/buildpack.toml",
            r#"[buildpack]
id = "a"
version = "0.0.0"
"#,
        );
        let buildpack_b = create_buildpack_file_with_name(
            "/b/buildpack.toml",
            r#"[buildpack]
id = "b"
version = "0.0.1"
"#,
        );
        match get_fixed_version(&vec![buildpack_a, buildpack_b]).unwrap_err() {
            Error::NotAllVersionsMatch(version_map) => {
                assert_eq!(
                    HashMap::from([
                        (
                            PathBuf::from("/a/buildpack.toml"),
                            BuildpackVersion {
                                major: 0,
                                minor: 0,
                                patch: 0
                            }
                        ),
                        (
                            PathBuf::from("/b/buildpack.toml"),
                            BuildpackVersion {
                                major: 0,
                                minor: 0,
                                patch: 1
                            }
                        )
                    ]),
                    version_map
                );
            }
            _ => panic!("Expected error NoFixedVersion"),
        };
    }

    #[test]
    fn test_update_buildpack_contents_with_new_version() {
        let toml = r#"[buildpack]
id = "test"
version = "0.0.0"
            "#;

        let mut buildpack_file = create_buildpack_file(toml);
        let next_version = BuildpackVersion {
            major: 1,
            minor: 0,
            patch: 0,
        };
        let updated_dependencies = HashSet::new();
        assert_eq!(
            update_buildpack_contents_with_new_version(
                &mut buildpack_file,
                &next_version,
                &updated_dependencies
            )
            .unwrap(),
            r#"[buildpack]
id = "test"
version = "1.0.0"
            "#
        );
    }

    #[test]
    fn test_update_buildpack_contents_with_new_version_and_order_groups_are_present() {
        let toml = r#"[buildpack]
id = "test"
version = "0.0.9"

[[order]]
[[order.group]]
id = "dep-a"
version = "0.0.9"

[[order.group]]
id = "dep-b"
version = "0.0.9"

[[order.group]]
id = "heroku/procfile"
version = "2.0.0"
optional = true
            "#;

        let mut buildpack_file = create_buildpack_file(toml);
        let next_version = BuildpackVersion {
            major: 0,
            minor: 0,
            patch: 10,
        };
        let updated_dependencies = HashSet::from([buildpack_id!("dep-a"), buildpack_id!("dep-b")]);
        assert_eq!(
            update_buildpack_contents_with_new_version(
                &mut buildpack_file,
                &next_version,
                &updated_dependencies
            )
            .unwrap(),
            r#"[buildpack]
id = "test"
version = "0.0.10"

[[order]]
[[order.group]]
id = "dep-a"
version = "0.0.10"

[[order.group]]
id = "dep-b"
version = "0.0.10"

[[order.group]]
id = "heroku/procfile"
version = "2.0.0"
optional = true
            "#
        );
    }

    #[test]
    fn test_promote_changelog_unreleased_to_version_with_existing_entries() {
        let mut changelog: Changelog = "\
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Added node version 18.15.0.
- Added yarn version 4.0.0-rc.2

## [0.8.16] - 2023-02-27

### Added

- Added node version 19.7.0, 19.6.1, 14.21.3, 16.19.1, 18.14.1, 18.14.2.
- Added node version 18.14.0, 19.6.0.

## [0.8.15] - 2023-02-26

### Added

- `name` is no longer a required field in package.json. ([#447](https://github.com/heroku/buildpacks-nodejs/pull/447))
- Added node version 19.5.0.

[unreleased]: https://github.com/heroku/buildpacks-nodejs/compare/v0.8.16...HEAD
[0.8.16]: https://github.com/heroku/buildpacks-nodejs/compare/v0.8.15...v0.8.16
[0.8.15]: https://github.com/heroku/buildpacks-nodejs/releases/tag/v/v0.8.15\n".parse().unwrap();

        let next_version = BuildpackVersion {
            major: 0,
            minor: 8,
            patch: 17,
        };
        let updated_dependencies = HashSet::new();
        let repository_url = "https://github.com/heroku/buildpacks-nodejs".to_string();
        let today = ReleaseDate::today();
        promote_changelog_unreleased_to_version(
            &mut changelog,
            &next_version,
            &repository_url,
            &updated_dependencies,
        )
        .unwrap();

        assert_eq!(changelog.to_string(), format!("\
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.8.17] - {today}

### Added

- Added node version 18.15.0.
- Added yarn version 4.0.0-rc.2

## [0.8.16] - 2023-02-27

### Added

- Added node version 19.7.0, 19.6.1, 14.21.3, 16.19.1, 18.14.1, 18.14.2.
- Added node version 18.14.0, 19.6.0.

## [0.8.15] - 2023-02-26

### Added

- `name` is no longer a required field in package.json. ([#447](https://github.com/heroku/buildpacks-nodejs/pull/447))
- Added node version 19.5.0.

[unreleased]: https://github.com/heroku/buildpacks-nodejs/compare/v0.8.17...HEAD
[0.8.17]: https://github.com/heroku/buildpacks-nodejs/compare/v0.8.16...v0.8.17
[0.8.16]: https://github.com/heroku/buildpacks-nodejs/compare/v0.8.15...v0.8.16
[0.8.15]: https://github.com/heroku/buildpacks-nodejs/releases/tag/v/v0.8.15\n"
        ));
    }

    #[test]
    fn test_promote_changelog_unreleased_to_version_with_no_entries() {
        let mut changelog: Changelog = "\
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

[unreleased]: https://github.com/heroku/buildpacks-nodejs\n"
            .parse()
            .unwrap();

        let next_version = BuildpackVersion {
            major: 0,
            minor: 8,
            patch: 17,
        };
        let updated_dependencies = HashSet::new();
        let repository_url = "https://github.com/heroku/buildpacks-nodejs".to_string();
        let today = ReleaseDate::today();
        promote_changelog_unreleased_to_version(
            &mut changelog,
            &next_version,
            &repository_url,
            &updated_dependencies,
        )
        .unwrap();

        assert_eq!(
            changelog.to_string(),
            format!(
                "\
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.8.17] - {today} [NO CHANGES]

[unreleased]: https://github.com/heroku/buildpacks-nodejs/compare/v0.8.17...HEAD
[0.8.17]: https://github.com/heroku/buildpacks-nodejs/releases/tag/v0.8.17\n"
            )
        );
    }

    #[test]
    fn test_promote_changelog_unreleased_to_version_with_existing_entries_and_updated_dependencies()
    {
        let mut changelog: Changelog = "\
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Added node version 18.15.0.
- Added yarn version 4.0.0-rc.2

## [0.8.16] - 2023-02-27

### Added

- Added node version 19.7.0, 19.6.1, 14.21.3, 16.19.1, 18.14.1, 18.14.2.
- Added node version 18.14.0, 19.6.0.

## [0.8.15] - 2023-02-26

### Added

- `name` is no longer a required field in package.json. ([#447](https://github.com/heroku/buildpacks-nodejs/pull/447))
- Added node version 19.5.0.

[unreleased]: https://github.com/heroku/buildpacks-nodejs/compare/v0.8.16...HEAD
[0.8.16]: https://github.com/heroku/buildpacks-nodejs/compare/v0.8.15...v0.8.16
[0.8.15]: https://github.com/heroku/buildpacks-nodejs/releases/tag/v/v0.8.15\n".parse().unwrap();

        let next_version = BuildpackVersion {
            major: 0,
            minor: 8,
            patch: 17,
        };
        let updated_dependencies = HashSet::from([buildpack_id!("b"), buildpack_id!("a")]);
        let repository_url = "https://github.com/heroku/buildpacks-nodejs".to_string();
        let today = ReleaseDate::today();
        promote_changelog_unreleased_to_version(
            &mut changelog,
            &next_version,
            &repository_url,
            &updated_dependencies,
        )
        .unwrap();

        assert_eq!(changelog.to_string(), format!("\
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.8.17] - {today}

### Added

- Added node version 18.15.0.
- Added yarn version 4.0.0-rc.2

### Changed

- Updated `a` to `0.8.17`.
- Updated `b` to `0.8.17`.

## [0.8.16] - 2023-02-27

### Added

- Added node version 19.7.0, 19.6.1, 14.21.3, 16.19.1, 18.14.1, 18.14.2.
- Added node version 18.14.0, 19.6.0.

## [0.8.15] - 2023-02-26

### Added

- `name` is no longer a required field in package.json. ([#447](https://github.com/heroku/buildpacks-nodejs/pull/447))
- Added node version 19.5.0.

[unreleased]: https://github.com/heroku/buildpacks-nodejs/compare/v0.8.17...HEAD
[0.8.17]: https://github.com/heroku/buildpacks-nodejs/compare/v0.8.16...v0.8.17
[0.8.16]: https://github.com/heroku/buildpacks-nodejs/compare/v0.8.15...v0.8.16
[0.8.15]: https://github.com/heroku/buildpacks-nodejs/releases/tag/v/v0.8.15\n"
        ));
    }

    #[test]
    fn test_promote_changelog_unreleased_to_version_with_no_entries_and_updated_dependencies() {
        let mut changelog: Changelog = "\
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.8.16] - 2023-02-27

### Added

- Added node version 19.7.0, 19.6.1, 14.21.3, 16.19.1, 18.14.1, 18.14.2.
- Added node version 18.14.0, 19.6.0.

## [0.8.15] - 2023-02-26

### Added

- `name` is no longer a required field in package.json. ([#447](https://github.com/heroku/buildpacks-nodejs/pull/447))
- Added node version 19.5.0.

[unreleased]: https://github.com/heroku/buildpacks-nodejs/compare/v0.8.16...HEAD
[0.8.16]: https://github.com/heroku/buildpacks-nodejs/compare/v0.8.15...v0.8.16
[0.8.15]: https://github.com/heroku/buildpacks-nodejs/releases/tag/v/v0.8.15\n".parse().unwrap();

        let next_version = BuildpackVersion {
            major: 0,
            minor: 8,
            patch: 17,
        };
        let updated_dependencies = HashSet::from([buildpack_id!("b"), buildpack_id!("a")]);
        let repository_url = "https://github.com/heroku/buildpacks-nodejs".to_string();
        let today = ReleaseDate::today();
        promote_changelog_unreleased_to_version(
            &mut changelog,
            &next_version,
            &repository_url,
            &updated_dependencies,
        )
        .unwrap();

        assert_eq!(changelog.to_string(), format!("\
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.8.17] - {today}

### Changed

- Updated `a` to `0.8.17`.
- Updated `b` to `0.8.17`.

## [0.8.16] - 2023-02-27

### Added

- Added node version 19.7.0, 19.6.1, 14.21.3, 16.19.1, 18.14.1, 18.14.2.
- Added node version 18.14.0, 19.6.0.

## [0.8.15] - 2023-02-26

### Added

- `name` is no longer a required field in package.json. ([#447](https://github.com/heroku/buildpacks-nodejs/pull/447))
- Added node version 19.5.0.

[unreleased]: https://github.com/heroku/buildpacks-nodejs/compare/v0.8.17...HEAD
[0.8.17]: https://github.com/heroku/buildpacks-nodejs/compare/v0.8.16...v0.8.17
[0.8.16]: https://github.com/heroku/buildpacks-nodejs/compare/v0.8.15...v0.8.16
[0.8.15]: https://github.com/heroku/buildpacks-nodejs/releases/tag/v/v0.8.15\n"
        ));
    }
    #[test]
    fn test_promote_changelog_unreleased_to_version_with_changed_entries_is_merged_with_updated_dependencies(
    ) {
        let mut changelog: Changelog = "\
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Added feature X

## [0.8.16] - 2023-02-27

### Added

- Added node version 19.7.0, 19.6.1, 14.21.3, 16.19.1, 18.14.1, 18.14.2.
- Added node version 18.14.0, 19.6.0.

## [0.8.15] - 2023-02-26

### Added

- `name` is no longer a required field in package.json. ([#447](https://github.com/heroku/buildpacks-nodejs/pull/447))
- Added node version 19.5.0.

[unreleased]: https://github.com/heroku/buildpacks-nodejs/compare/v0.8.16...HEAD
[0.8.16]: https://github.com/heroku/buildpacks-nodejs/compare/v0.8.15...v0.8.16
[0.8.15]: https://github.com/heroku/buildpacks-nodejs/releases/tag/v/v0.8.15\n".parse().unwrap();

        let next_version = BuildpackVersion {
            major: 0,
            minor: 8,
            patch: 17,
        };
        let updated_dependencies = HashSet::from([buildpack_id!("b"), buildpack_id!("a")]);
        let repository_url = "https://github.com/heroku/buildpacks-nodejs".to_string();
        let today = ReleaseDate::today();
        promote_changelog_unreleased_to_version(
            &mut changelog,
            &next_version,
            &repository_url,
            &updated_dependencies,
        )
        .unwrap();

        assert_eq!(changelog.to_string(), format!("\
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.8.17] - {today}

### Changed

- Added feature X
- Updated `a` to `0.8.17`.
- Updated `b` to `0.8.17`.

## [0.8.16] - 2023-02-27

### Added

- Added node version 19.7.0, 19.6.1, 14.21.3, 16.19.1, 18.14.1, 18.14.2.
- Added node version 18.14.0, 19.6.0.

## [0.8.15] - 2023-02-26

### Added

- `name` is no longer a required field in package.json. ([#447](https://github.com/heroku/buildpacks-nodejs/pull/447))
- Added node version 19.5.0.

[unreleased]: https://github.com/heroku/buildpacks-nodejs/compare/v0.8.17...HEAD
[0.8.17]: https://github.com/heroku/buildpacks-nodejs/compare/v0.8.16...v0.8.17
[0.8.16]: https://github.com/heroku/buildpacks-nodejs/compare/v0.8.15...v0.8.16
[0.8.15]: https://github.com/heroku/buildpacks-nodejs/releases/tag/v/v0.8.15\n"
        ));
    }

    fn create_buildpack_file(contents: &str) -> BuildpackFile {
        create_buildpack_file_with_name("/path/to/test/buildpack.toml", contents)
    }

    fn create_buildpack_file_with_name(name: &str, contents: &str) -> BuildpackFile {
        BuildpackFile {
            path: PathBuf::from(name),
            document: Document::from_str(contents).unwrap(),
        }
    }
}
