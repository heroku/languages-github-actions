use crate::buildpacks::find_releasable_buildpacks;
use crate::changelog::Changelog;
use crate::commands::generate_changelog::errors::Error;
use crate::github::actions;
use clap::Parser;
use libcnb_data::buildpack::BuildpackId;
use libcnb_package::read_buildpack_data;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

type Result<T> = std::result::Result<T, Error>;

#[derive(Parser, Debug)]
#[command(author, version, about = "Generates a changelog from one or more buildpacks in a project", long_about = None, disable_version_flag = true)]
pub(crate) struct GenerateChangelogArgs {
    #[arg(long, group = "section")]
    unreleased: bool,
    #[arg(long, group = "section")]
    version: Option<String>,
    #[arg(long)]
    path: Option<String>,
}

enum ChangelogEntryType {
    Unreleased,
    Version(String),
}

enum ChangelogEntry {
    VersionNotPresent,
    Empty,
    Changes(String),
}

pub(crate) fn execute(args: GenerateChangelogArgs) -> Result<()> {
    let working_dir =
        get_working_dir_from(args.path.map(PathBuf::from)).map_err(Error::GetWorkingDir)?;

    let buildpack_dirs = find_releasable_buildpacks(&working_dir)
        .map_err(|e| Error::FindingBuildpacks(working_dir.clone(), e))?;

    let changelog_entry_type = match args.version {
        Some(version) => ChangelogEntryType::Version(version),
        None => ChangelogEntryType::Unreleased,
    };

    let changes_by_buildpack = buildpack_dirs
        .iter()
        .map(|dir| {
            read_buildpack_data(dir)
                .map_err(Error::GetBuildpackId)
                .map(|data| data.buildpack_descriptor.buildpack().id.clone())
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
) -> Result<ChangelogEntry> {
    let contents =
        std::fs::read_to_string(path).map_err(|e| Error::ReadingChangelog(path.clone(), e))?;
    let changelog = Changelog::try_from(contents.as_str())
        .map_err(|e| Error::ParsingChangelog(path.clone(), e))?;
    Ok(match changelog_entry_type {
        ChangelogEntryType::Unreleased => changelog
            .unreleased
            .map_or(ChangelogEntry::Empty, ChangelogEntry::Changes),

        ChangelogEntryType::Version(version) => {
            changelog
                .releases
                .get(version)
                .map_or(ChangelogEntry::VersionNotPresent, |entry| {
                    if entry.body.is_empty() {
                        ChangelogEntry::Empty
                    } else {
                        ChangelogEntry::Changes(entry.body.clone())
                    }
                })
        }
    })
}

fn get_working_dir_from(path: Option<PathBuf>) -> std::io::Result<PathBuf> {
    let current_dir = std::env::current_dir()?;
    Ok(match path {
        Some(value) => {
            if value.is_absolute() {
                value
            } else {
                current_dir.join(value)
            }
        }
        None => current_dir,
    })
}

fn generate_changelog(changes_by_buildpack: &HashMap<BuildpackId, ChangelogEntry>) -> String {
    let changelog = changes_by_buildpack
        .iter()
        .map(|(buildpack_id, changes)| (buildpack_id.to_string(), changes))
        .collect::<BTreeMap<_, _>>()
        .into_iter()
        .filter_map(|(buildpack_id, changes)| match changes {
            ChangelogEntry::Empty => Some(format!("## {buildpack_id}\n\n- No changes.")),
            ChangelogEntry::Changes(value) => Some(format!("## {buildpack_id}\n\n{value}")),
            ChangelogEntry::VersionNotPresent => None,
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    format!("{}\n\n", changelog.trim())
}

#[cfg(test)]
mod test {
    use crate::commands::generate_changelog::command::{generate_changelog, ChangelogEntry};
    use libcnb_data::buildpack_id;
    use std::collections::HashMap;

    #[test]
    fn test_generating_changelog() {
        let values = HashMap::from([
            (
                buildpack_id!("c"),
                ChangelogEntry::Changes("- change c.1".to_string()),
            ),
            (
                buildpack_id!("a"),
                ChangelogEntry::Changes("- change a.1\n- change a.2".to_string()),
            ),
            (buildpack_id!("b"), ChangelogEntry::VersionNotPresent),
            (buildpack_id!("d"), ChangelogEntry::Empty),
        ]);

        assert_eq!(
            generate_changelog(&values),
            r"## a

- change a.1
- change a.2

## c

- change c.1

## d

- No changes.

"
        );
    }
}
