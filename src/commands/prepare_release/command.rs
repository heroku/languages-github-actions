use crate::buildpacks::find_releasable_buildpacks;
use crate::changelog::{generate_release_declarations, Changelog, ReleaseEntry};
use crate::commands::prepare_release::errors::Error;
use crate::commands::resolve_path;
use crate::github::actions;
use chrono::{DateTime, Utc};
use clap::{Parser, ValueEnum};
use indexmap::IndexMap;
use libcnb_data::buildpack::{BuildpackId, BuildpackVersion};
use semver::{BuildMetadata, Prerelease, Version};
use std::collections::{HashMap, HashSet};
use std::fs::write;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use toml_edit::{value, ArrayOfTables, Document, Table};
use uriparse::URI;

type Result<T> = std::result::Result<T, Error>;

#[derive(Parser, Debug)]
#[command(author, version, about = "Bumps the version of each detected buildpack and adds an entry for any unreleased changes from the changelog", long_about = None)]
pub(crate) struct PrepareReleaseArgs {
    #[arg(long)]
    pub(crate) working_dir: Option<PathBuf>,
    #[arg(long, value_enum)]
    pub(crate) bump: BumpCoordinate,
    #[arg(long)]
    pub(crate) repository_url: String,
    #[arg(long)]
    pub(crate) declarations_starting_version: Option<String>,
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
    let working_dir = std::env::current_dir()
        .map(|base| {
            args.working_dir
                .map_or(base.clone(), |path| resolve_path(&path, &base))
        })
        .map_err(Error::ResolveWorkingDir)?;

    let repository_url = URI::try_from(args.repository_url.as_str())
        .map(URI::into_owned)
        .map_err(|e| Error::InvalidRepositoryUrl(args.repository_url.clone(), e))?;

    let declarations_starting_version = args
        .declarations_starting_version
        .map(|value| {
            value
                .parse::<Version>()
                .map_err(|e| Error::InvalidDeclarationsStartingVersion(value, e))
        })
        .transpose()?;

    let buildpack_dirs =
        find_releasable_buildpacks(&working_dir).map_err(Error::FindReleasableBuildpacks)?;

    if buildpack_dirs.is_empty() {
        Err(Error::NoBuildpacksFound(working_dir))?;
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

    for (mut buildpack_file, changelog_file) in buildpack_files.into_iter().zip(changelog_files) {
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

        let new_changelog = promote_changelog_unreleased_to_version(
            &changelog_file.changelog,
            &next_version,
            &Utc::now(),
            &updated_dependencies,
        );

        let release_declarations = generate_release_declarations(
            &new_changelog,
            repository_url.to_string(),
            &declarations_starting_version,
        );

        let changelog_contents = format!("{new_changelog}\n{release_declarations}\n");

        write(&changelog_file.path, changelog_contents)
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
    let changelog = Changelog::try_from(contents.as_str())
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
    changelog: &Changelog,
    version: &BuildpackVersion,
    date: &DateTime<Utc>,
    updated_dependencies: &HashSet<BuildpackId>,
) -> Changelog {
    let updated_dependencies_text = if updated_dependencies.is_empty() {
        None
    } else {
        let mut updated_dependencies_bullet_points = updated_dependencies
            .iter()
            .map(|id| format!("- Updated `{id}` to `{version}`."))
            .collect::<Vec<_>>();
        updated_dependencies_bullet_points.sort();
        Some(updated_dependencies_bullet_points.join("\n"))
    };

    let changes_with_dependencies = (&changelog.unreleased, &updated_dependencies_text);

    let body = if let (Some(changes), Some(dependencies)) = changes_with_dependencies {
        merge_existing_changelog_entries_with_dependency_changes(changes, dependencies)
    } else if let (Some(changes), None) = changes_with_dependencies {
        changes.clone()
    } else if let (None, Some(dependencies)) = changes_with_dependencies {
        format!("### Changed\n\n{dependencies}")
    } else {
        "- No changes.".to_string()
    };

    let new_release_entry = ReleaseEntry {
        version: Version {
            major: version.major,
            minor: version.minor,
            patch: version.patch,
            pre: Prerelease::default(),
            build: BuildMetadata::default(),
        },
        date: *date,
        body,
    };

    let mut releases = IndexMap::from([(version.to_string(), new_release_entry)]);
    for (id, entry) in &changelog.releases {
        releases.insert(id.clone(), entry.clone());
    }
    Changelog {
        unreleased: None,
        releases,
    }
}

fn merge_existing_changelog_entries_with_dependency_changes(
    changelog_entries: &str,
    updated_dependencies: &str,
) -> String {
    if changelog_entries.contains("### Changed") {
        changelog_entries
            .split("### ")
            .map(|entry| {
                if entry.starts_with("Changed") {
                    format!("{}\n{}\n\n", entry.trim_end(), updated_dependencies)
                } else {
                    entry.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("### ")
    } else {
        format!(
            "{}\n\n### Changed\n\n{}",
            changelog_entries.trim_end(),
            updated_dependencies
        )
    }
}

#[cfg(test)]
mod test {
    use crate::changelog::{Changelog, ReleaseEntry};
    use crate::commands::prepare_release::command::{
        get_fixed_version, promote_changelog_unreleased_to_version,
        update_buildpack_contents_with_new_version, BuildpackFile,
    };
    use crate::commands::prepare_release::errors::Error;
    use chrono::{TimeZone, Utc};
    use indexmap::IndexMap;
    use libcnb_data::buildpack::BuildpackVersion;
    use libcnb_data::buildpack_id;
    use semver::Version;
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
        let release_entry_0_8_16 = ReleaseEntry {
            version: "0.8.16".parse::<Version>().unwrap(),
            date: Utc.with_ymd_and_hms(2023, 2, 27, 0, 0, 0).unwrap(),
            body: "- Added node version 19.7.0, 19.6.1, 14.21.3, 16.19.1, 18.14.1, 18.14.2.\n- Added node version 18.14.0, 19.6.0.".to_string()
        };

        let release_entry_0_8_15 = ReleaseEntry {
            version: "0.8.15".parse::<Version>().unwrap(),
            date: Utc.with_ymd_and_hms(2023, 2, 27, 0, 0, 0).unwrap(),
            body: "- `name` is no longer a required field in package.json. ([#447](https://github.com/heroku/buildpacks-nodejs/pull/447))\n- Added node version 19.5.0.".to_string()
        };

        let changelog = Changelog {
            unreleased: Some(
                "- Added node version 18.15.0.\n- Added yarn version 4.0.0-rc.2".to_string(),
            ),
            releases: IndexMap::from([
                ("0.8.16".to_string(), release_entry_0_8_16.clone()),
                ("0.8.15".to_string(), release_entry_0_8_15.clone()),
            ]),
        };

        assert_eq!(
            changelog.unreleased,
            Some("- Added node version 18.15.0.\n- Added yarn version 4.0.0-rc.2".to_string())
        );
        assert_eq!(changelog.releases.get("0.8.17"), None);
        assert_eq!(
            changelog.releases.get("0.8.16"),
            Some(&release_entry_0_8_16)
        );
        assert_eq!(
            changelog.releases.get("0.8.15"),
            Some(&release_entry_0_8_15)
        );

        let next_version = BuildpackVersion {
            major: 0,
            minor: 8,
            patch: 17,
        };
        let date = Utc.with_ymd_and_hms(2023, 6, 16, 0, 0, 0).unwrap();
        let updated_dependencies = HashSet::new();
        let changelog = promote_changelog_unreleased_to_version(
            &changelog,
            &next_version,
            &date,
            &updated_dependencies,
        );

        assert_eq!(changelog.unreleased, None);
        assert_eq!(
            changelog.releases.get("0.8.17"),
            Some(&ReleaseEntry {
                version: "0.8.17".parse::<Version>().unwrap(),
                date,
                body: "- Added node version 18.15.0.\n- Added yarn version 4.0.0-rc.2".to_string()
            })
        );
        assert_eq!(
            changelog.releases.get("0.8.16"),
            Some(&release_entry_0_8_16)
        );
        assert_eq!(
            changelog.releases.get("0.8.15"),
            Some(&release_entry_0_8_15)
        );
    }

    #[test]
    fn test_promote_changelog_unreleased_to_version_with_no_entries() {
        let changelog = Changelog {
            unreleased: None,
            releases: IndexMap::new(),
        };

        assert_eq!(changelog.unreleased, None);
        assert_eq!(changelog.releases.get("0.8.17"), None);

        let next_version = BuildpackVersion {
            major: 0,
            minor: 8,
            patch: 17,
        };
        let date = Utc.with_ymd_and_hms(2023, 6, 16, 0, 0, 0).unwrap();
        let updated_dependencies = HashSet::new();
        let changelog = promote_changelog_unreleased_to_version(
            &changelog,
            &next_version,
            &date,
            &updated_dependencies,
        );

        assert_eq!(changelog.unreleased, None);
        assert_eq!(
            changelog.releases.get("0.8.17"),
            Some(&ReleaseEntry {
                version: "0.8.17".parse::<Version>().unwrap(),
                date,
                body: "- No changes.".to_string()
            })
        );
    }

    #[test]
    fn test_promote_changelog_unreleased_to_version_with_existing_entries_and_updated_dependencies()
    {
        let release_entry_0_8_16 = ReleaseEntry {
            version: "0.8.16".parse::<Version>().unwrap(),
            date: Utc.with_ymd_and_hms(2023, 2, 27, 0, 0, 0).unwrap(),
            body: "### Added\n\n- Added node version 19.7.0, 19.6.1, 14.21.3, 16.19.1, 18.14.1, 18.14.2.\n- Added node version 18.14.0, 19.6.0.".to_string()
        };

        let release_entry_0_8_15 = ReleaseEntry {
            version: "0.8.15".parse::<Version>().unwrap(),
            date: Utc.with_ymd_and_hms(2023, 2, 27, 0, 0, 0).unwrap(),
            body: "### Changed\n\n- `name` is no longer a required field in package.json. ([#447](https://github.com/heroku/buildpacks-nodejs/pull/447))\n\n### Added\n\n- Added node version 19.5.0.".to_string()
        };

        let changelog = Changelog {
            unreleased: Some(
                "### Added\n\n- Added node version 18.15.0.\n- Added yarn version 4.0.0-rc.2"
                    .to_string(),
            ),
            releases: IndexMap::from([
                ("0.8.16".to_string(), release_entry_0_8_16.clone()),
                ("0.8.15".to_string(), release_entry_0_8_15.clone()),
            ]),
        };

        assert_eq!(
            changelog.unreleased,
            Some(
                "### Added\n\n- Added node version 18.15.0.\n- Added yarn version 4.0.0-rc.2"
                    .to_string()
            )
        );
        assert_eq!(changelog.releases.get("0.8.17"), None);
        assert_eq!(
            changelog.releases.get("0.8.16"),
            Some(&release_entry_0_8_16)
        );
        assert_eq!(
            changelog.releases.get("0.8.15"),
            Some(&release_entry_0_8_15)
        );

        let next_version = BuildpackVersion {
            major: 0,
            minor: 8,
            patch: 17,
        };
        let date = Utc.with_ymd_and_hms(2023, 6, 16, 0, 0, 0).unwrap();
        let updated_dependencies = HashSet::from([buildpack_id!("b"), buildpack_id!("a")]);
        let changelog = promote_changelog_unreleased_to_version(
            &changelog,
            &next_version,
            &date,
            &updated_dependencies,
        );

        assert_eq!(changelog.unreleased, None);
        assert_eq!(
            changelog.releases.get("0.8.17"),
            Some(&ReleaseEntry {
                version: "0.8.17".parse::<Version>().unwrap(),
                date,
                body: "### Added\n\n- Added node version 18.15.0.\n- Added yarn version 4.0.0-rc.2\n\n### Changed\n\n- Updated `a` to `0.8.17`.\n- Updated `b` to `0.8.17`.".to_string()
            })
        );
        assert_eq!(
            changelog.releases.get("0.8.16"),
            Some(&release_entry_0_8_16)
        );
        assert_eq!(
            changelog.releases.get("0.8.15"),
            Some(&release_entry_0_8_15)
        );
    }

    #[test]
    fn test_promote_changelog_unreleased_to_version_with_no_entries_and_updated_dependencies() {
        let changelog = Changelog {
            unreleased: None,
            releases: IndexMap::new(),
        };

        assert_eq!(changelog.unreleased, None);
        assert_eq!(changelog.releases.get("0.8.17"), None);

        let next_version = BuildpackVersion {
            major: 0,
            minor: 8,
            patch: 17,
        };
        let date = Utc.with_ymd_and_hms(2023, 6, 16, 0, 0, 0).unwrap();
        let updated_dependencies = HashSet::from([buildpack_id!("a"), buildpack_id!("b")]);
        let changelog = promote_changelog_unreleased_to_version(
            &changelog,
            &next_version,
            &date,
            &updated_dependencies,
        );

        assert_eq!(changelog.unreleased, None);
        assert_eq!(
            changelog.releases.get("0.8.17"),
            Some(&ReleaseEntry {
                version: "0.8.17".parse::<Version>().unwrap(),
                date,
                body: "### Changed\n\n- Updated `a` to `0.8.17`.\n- Updated `b` to `0.8.17`."
                    .to_string()
            })
        );
    }

    #[test]
    fn test_promote_changelog_unreleased_to_version_with_changed_entries_is_merged_with_updated_dependencies(
    ) {
        let changelog = Changelog {
            unreleased: Some(
                r"
- Entry not under a header

### Added

- Added node version 18.15.0.
- Added yarn version 4.0.0-rc.2

### Changed

- Lowed limits

### Removed

- Dropped all deprecated methods
                "
                .trim()
                .to_string(),
            ),
            releases: IndexMap::new(),
        };

        let next_version = BuildpackVersion {
            major: 0,
            minor: 8,
            patch: 17,
        };
        let date = Utc.with_ymd_and_hms(2023, 6, 16, 0, 0, 0).unwrap();
        let updated_dependencies = HashSet::from([buildpack_id!("b"), buildpack_id!("a")]);
        let changelog = promote_changelog_unreleased_to_version(
            &changelog,
            &next_version,
            &date,
            &updated_dependencies,
        );

        assert_eq!(changelog.unreleased, None);
        assert_eq!(
            changelog.releases.get("0.8.17").unwrap().body,
            r"
- Entry not under a header

### Added

- Added node version 18.15.0.
- Added yarn version 4.0.0-rc.2

### Changed

- Lowed limits
- Updated `a` to `0.8.17`.
- Updated `b` to `0.8.17`.

### Removed

- Dropped all deprecated methods
            "
            .trim()
            .to_string()
        );
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
