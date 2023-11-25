use crate::buildpacks::{find_releasable_buildpacks, read_buildpack_descriptor};
use crate::commands::generate_changelog::errors::Error;
use crate::github::actions;
use clap::Parser;
use keep_a_changelog::{Changelog, Changes};
use libcnb_data::buildpack::BuildpackId;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

type Result<T> = std::result::Result<T, Error>;

#[derive(Parser, Debug)]
#[command(author, version, about = "Generates a changelog from one or more buildpacks in a project", long_about = None, disable_version_flag = true)]
pub(crate) struct GenerateChangelogArgs {
    #[arg(long, group = "section")]
    pub(crate) unreleased: bool,
    #[arg(long, group = "section")]
    pub(crate) version: Option<String>,
}

enum ChangelogEntryType {
    Unreleased,
    Version(keep_a_changelog::Version),
}

pub(crate) fn execute(args: GenerateChangelogArgs) -> Result<()> {
    let current_dir = std::env::current_dir().map_err(Error::GetCurrentDir)?;
    let buildpack_dirs =
        find_releasable_buildpacks(&current_dir).map_err(Error::FindReleasableBuildpacks)?;

    let changelog_entry_type = match args.version {
        Some(version) => {
            ChangelogEntryType::Version(version.parse().map_err(Error::InvalidVersion)?)
        }
        None => ChangelogEntryType::Unreleased,
    };

    let changes_by_buildpack = buildpack_dirs
        .iter()
        .map(|dir| {
            read_buildpack_descriptor(dir)
                .map_err(Error::ReadBuildpackDescriptor)
                .map(|buildpack_descriptor| buildpack_descriptor.buildpack().id.clone())
                .and_then(|buildpack_id| {
                    read_changelog_entry(&dir.join("CHANGELOG.md"), &changelog_entry_type)
                        .map(|contents| (buildpack_id, contents))
                })
        })
        .collect::<Result<HashMap<_, _>>>()?;

    let changelog = generate_changelog(&changes_by_buildpack);

    actions::set_output("changelog", changelog).map_err(Error::SetActionOutput)?;

    Ok(())
}

fn read_changelog_entry(
    path: &PathBuf,
    changelog_entry_type: &ChangelogEntryType,
) -> Result<Option<Changes>> {
    let contents =
        std::fs::read_to_string(path).map_err(|e| Error::ReadingChangelog(path.clone(), e))?;
    let changelog: Changelog = contents
        .parse()
        .map_err(|e| Error::ParsingChangelog(path.clone(), e))?;
    Ok(match changelog_entry_type {
        ChangelogEntryType::Unreleased => Some(changelog.unreleased.changes),
        ChangelogEntryType::Version(version) => changelog
            .releases
            .get_version(version)
            .map(|release| release.changes.clone()),
    })
}

fn generate_changelog(changes_by_buildpack: &HashMap<BuildpackId, Option<Changes>>) -> String {
    let (buildpacks_with_no_changes, buildpacks_with_changes): (Vec<_>, Vec<_>) =
        changes_by_buildpack
            .iter()
            .filter_map(|(buildpack_id, changes)| {
                changes
                    .as_ref()
                    .map(|value| (buildpack_id.to_string(), value))
            })
            .collect::<BTreeMap<_, _>>()
            .into_iter()
            .partition(|(_, changes)| changes.is_empty());

    let notable_changes = buildpacks_with_changes
        .into_iter()
        .map(|(buildpack_id, changes)| {
            let mut section = String::new();
            section.push_str(&format!("## {buildpack_id}\n\n"));
            for (change_group, items) in changes {
                section.push_str(&format!("### {change_group}\n"));
                for item in items {
                    section.push_str(&format!("\n- {item}"));
                }
            }
            section
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    let extra_details = buildpacks_with_no_changes
        .iter()
        .map(|(buildpack_id, _)| buildpack_id.to_string())
        .collect::<Vec<_>>();

    if extra_details.is_empty() {
        format!("{}\n", notable_changes.trim())
    } else {
        format!("{}\n\n> The following buildpacks had their versions bumped but contained no changes: {}\n", notable_changes.trim(), extra_details.join(", "))
    }
}

#[cfg(test)]
mod test {
    use crate::commands::generate_changelog::command::generate_changelog;
    use keep_a_changelog::{ChangeGroup, Changes};
    use libcnb_data::buildpack_id;
    use std::collections::HashMap;

    fn changes(items: Vec<String>) -> Changes {
        let mut unreleased = keep_a_changelog::Unreleased::default();
        for item in items {
            unreleased.add(ChangeGroup::Changed, item);
        }
        unreleased.changes
    }

    #[test]
    fn test_generating_changelog_with_buildpacks_containing_no_changes() {
        let values = HashMap::from([
            (
                buildpack_id!("c"),
                Some(changes(vec!["change c.1".to_string()])),
            ),
            (
                buildpack_id!("a"),
                Some(changes(vec![
                    "change a.1".to_string(),
                    "change a.2".to_string(),
                ])),
            ),
            (buildpack_id!("b"), None),
            (buildpack_id!("d"), Some(changes(vec![]))),
            (buildpack_id!("e"), Some(changes(vec![]))),
        ]);

        assert_eq!(
            generate_changelog(&values),
            "\
## a

### Changed

- change a.1
- change a.2

## c

### Changed

- change c.1

> The following buildpacks had their versions bumped but contained no changes: d, e
"
        );
    }

    #[test]
    fn test_generating_changelog_with_buildpacks_that_all_have_changes() {
        let values = HashMap::from([
            (
                buildpack_id!("c"),
                Some(changes(vec!["change c.1".to_string()])),
            ),
            (
                buildpack_id!("a"),
                Some(changes(vec![
                    "change a.1".to_string(),
                    "change a.2".to_string(),
                ])),
            ),
            (buildpack_id!("b"), None),
        ]);

        assert_eq!(
            generate_changelog(&values),
            "\
## a

### Changed

- change a.1
- change a.2

## c

### Changed

- change c.1
"
        );
    }
}
