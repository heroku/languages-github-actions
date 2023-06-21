use chrono::{DateTime, LocalResult, TimeZone, Utc};
use indexmap::IndexMap;
use lazy_static::lazy_static;
use markdown::mdast::Node;
use markdown::{to_mdast, ParseOptions};
use regex::Regex;
use semver::Version;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::num::ParseIntError;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Changelog {
    pub(crate) unreleased: Option<String>,
    pub(crate) releases: IndexMap<String, ReleaseEntry>,
}

impl TryFrom<&str> for Changelog {
    type Error = ChangelogError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        lazy_static! {
            static ref UNRELEASED_HEADER: Regex =
                Regex::new(r"(?i)^\[?unreleased]?$").expect("Should be a valid regex");
            static ref VERSION_HEADER: Regex =
                Regex::new(r"^\[?(\d+\.\d+\.\d+)]?.*(\d{4})[-/](\d{2})[-/](\d{2})")
                    .expect("Should be a valid regex");
        }

        let changelog_ast =
            to_mdast(value, &ParseOptions::default()).map_err(ChangelogError::Parse)?;

        let mut current_header: Option<String> = None;
        let mut headers: Vec<String> = vec![];
        let mut body_nodes_by_header: HashMap<String, Vec<&Node>> = HashMap::new();

        if let Node::Root(root) = changelog_ast {
            for child in &root.children {
                if let Node::Heading(heading) = child {
                    match heading.depth.cmp(&2) {
                        Ordering::Equal => {
                            headers.push(child.to_string());
                            current_header = Some(child.to_string());
                        }
                        Ordering::Less => {
                            current_header = None;
                        }
                        _ => {
                            if let Some(header) = &current_header {
                                let body_nodes = body_nodes_by_header
                                    .entry(header.clone())
                                    .or_insert_with(Vec::new);
                                body_nodes.push(child);
                            }
                        }
                    }
                } else if let Node::Definition(_) = child {
                    // ignore any defined links, these will be regenerated at display time
                } else if let Some(header) = &current_header {
                    let body_nodes = body_nodes_by_header
                        .entry(header.clone())
                        .or_insert_with(Vec::new);
                    body_nodes.push(child);
                }
            }

            let mut unreleased = None;
            let mut releases = IndexMap::new();

            for header in headers {
                let empty_nodes = vec![];
                let body_nodes = body_nodes_by_header.get(&header).unwrap_or(&empty_nodes);

                let start = body_nodes
                    .iter()
                    .next()
                    .map(|node| node.position().map(|position| position.start.offset))
                    .unwrap_or_default();
                let end = body_nodes
                    .iter()
                    .last()
                    .map(|node| node.position().map(|position| position.end.offset))
                    .unwrap_or_default();

                let body = if let (Some(start), Some(end)) = (start, end) {
                    &value[start..end]
                } else {
                    ""
                };

                let body = body.trim().to_string();

                if UNRELEASED_HEADER.is_match(&header) && !body.is_empty() {
                    unreleased = Some(body);
                } else if let Some(captures) = VERSION_HEADER.captures(&header) {
                    let version = captures[1]
                        .parse::<Version>()
                        .map_err(ChangelogError::ParseVersion)?;
                    let year = captures[2]
                        .parse::<i32>()
                        .map_err(ChangelogError::ParseReleaseEntryYear)?;
                    let month = captures[3]
                        .parse::<u32>()
                        .map_err(ChangelogError::ParseReleaseEntryMonth)?;
                    let day = captures[4]
                        .parse::<u32>()
                        .map_err(ChangelogError::ParseReleaseEntryDay)?;
                    let date = match Utc.with_ymd_and_hms(year, month, day, 0, 0, 0) {
                        LocalResult::None => Err(ChangelogError::InvalidReleaseDate),
                        LocalResult::Single(value) => Ok(value),
                        LocalResult::Ambiguous(_, _) => Err(ChangelogError::AmbiguousReleaseDate),
                    }?;
                    releases.insert(
                        version.to_string(),
                        ReleaseEntry {
                            version,
                            body,
                            date,
                        },
                    );
                }
            }

            Ok(Changelog {
                unreleased,
                releases,
            })
        } else {
            Err(ChangelogError::NoRootNode)
        }
    }
}

impl Display for Changelog {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            r#"
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
        "#
            .trim()
        )?;

        if let Some(unreleased) = &self.unreleased {
            write!(f, "\n\n## [Unreleased]\n\n{}", unreleased.trim())?;
        } else {
            write!(f, "\n\n## [Unreleased]")?;
        }

        for entry in self.releases.values() {
            write!(
                f,
                "\n\n## [{}] - {}",
                entry.version,
                entry.date.format("%Y-%m-%d")
            )?;
            if !entry.body.is_empty() {
                write!(f, "\n\n{}", entry.body.trim())?;
            }
        }

        writeln!(f)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct ReleaseEntry {
    pub(crate) version: Version,
    pub(crate) date: DateTime<Utc>,
    pub(crate) body: String,
}

#[derive(Debug)]
pub(crate) enum ChangelogError {
    NoRootNode,
    Parse(String),
    ParseVersion(semver::Error),
    ParseReleaseEntryYear(ParseIntError),
    ParseReleaseEntryMonth(ParseIntError),
    ParseReleaseEntryDay(ParseIntError),
    InvalidReleaseDate,
    AmbiguousReleaseDate,
}

impl Display for ChangelogError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ChangelogError::NoRootNode => {
                write!(f, "No root node in changelog markdown")
            }
            ChangelogError::Parse(error) => {
                write!(f, "Could not parse changelog - {error}")
            }
            ChangelogError::ParseVersion(error) => {
                write!(f, "Invalid semver version in release entry - {error}")
            }
            ChangelogError::ParseReleaseEntryYear(error) => {
                write!(f, "Invalid year in release entry - {error}")
            }
            ChangelogError::ParseReleaseEntryMonth(error) => {
                write!(f, "Invalid month in release entry - {error}")
            }
            ChangelogError::ParseReleaseEntryDay(error) => {
                write!(f, "Invalid day in release entry - {error}")
            }
            ChangelogError::InvalidReleaseDate => {
                write!(f, "Invalid date in release entry")
            }
            ChangelogError::AmbiguousReleaseDate => {
                write!(f, "Ambiguous date in release entry")
            }
        }
    }
}

pub(crate) fn generate_release_declarations<S: Into<String>>(
    changelog: &Changelog,
    repository: S,
    starting_with_version: &Option<Version>,
) -> String {
    let repository = repository.into();

    let mut versions = changelog.releases.values().filter_map(|release| {
        if let Some(starting_version) = &starting_with_version {
            if starting_version.le(&release.version) {
                Some(&release.version)
            } else {
                None
            }
        } else {
            Some(&release.version)
        }
    });

    let mut declarations = vec![];
    let mut previous_version = versions.next();

    declarations.push(if let Some(version) = previous_version {
        format!("[unreleased]: {repository}/compare/v{version}...HEAD")
    } else {
        format!("[unreleased]: {repository}")
    });

    for next_version in versions {
        if let Some(version) = previous_version {
            declarations.push(format!(
                "[{version}]: {repository}/compare/v{next_version}...v{version}"
            ));
        }
        previous_version = Some(next_version)
    }

    if let Some(version) = previous_version {
        declarations.push(format!("[{version}]: {repository}/releases/tag/v{version}"));
    }

    declarations.join("\n")
}

#[cfg(test)]
mod test {
    use crate::changelog::{generate_release_declarations, Changelog};
    use chrono::{TimeZone, Utc};
    use semver::Version;

    #[test]
    fn test_keep_a_changelog_unreleased_entry_with_changes_parsing() {
        let changelog = Changelog::try_from("## [Unreleased]\n\n- Some changes").unwrap();
        assert_eq!(changelog.unreleased, Some("- Some changes".to_string()));
    }

    #[test]
    fn test_blank_release_0_5_5_entry_from_jvm_repo() {
        let changelog = Changelog::try_from(
            "## [Unreleased]

## [0.6.0] 2022/01/05

- Switch to BSD 3-Clause License
- Upgrade to libcnb version 0.4.0
- Updated function runtime to 1.0.5

## [0.5.5] 2021/10/19

## [0.5.4] 2021/09/30

- Updated function runtime to 1.0.3",
        )
        .unwrap();
        assert_eq!(changelog.releases.get("0.5.5").unwrap().body, "");
        assert_eq!(
            changelog.to_string(),
            "# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.6.0] - 2022-01-05

- Switch to BSD 3-Clause License
- Upgrade to libcnb version 0.4.0
- Updated function runtime to 1.0.5

## [0.5.5] - 2021-10-19

## [0.5.4] - 2021-09-30

- Updated function runtime to 1.0.3
"
        )
    }

    #[test]
    fn test_keep_a_changelog_unreleased_entry_with_no_changes_parsing() {
        let changelog = Changelog::try_from(KEEP_A_CHANGELOG_1_0_0).unwrap();
        assert_eq!(changelog.unreleased, None);
    }

    #[test]
    fn test_keep_a_changelog_release_entry_parsing() {
        let changelog = Changelog::try_from(KEEP_A_CHANGELOG_1_0_0).unwrap();
        let release_entry = changelog.releases.get("1.1.1").unwrap();
        assert_eq!(release_entry.version, "1.1.1".parse::<Version>().unwrap());
        assert_eq!(
            release_entry.date,
            Utc.with_ymd_and_hms(2023, 3, 5, 0, 0, 0).unwrap()
        );
        assert_eq!(
            release_entry.body,
            r#"### Added

- Arabic translation (#444).
- v1.1 French translation.
- v1.1 Dutch translation (#371).
- v1.1 Russian translation (#410).
- v1.1 Japanese translation (#363).
- v1.1 Norwegian Bokmål translation (#383).
- v1.1 "Inconsistent Changes" Turkish translation (#347).
- Default to most recent versions available for each languages
- Display count of available translations (26 to date!)
- Centralize all links into `/data/links.json` so they can be updated easily

### Fixed

- Improve French translation (#377).
- Improve id-ID translation (#416).
- Improve Persian translation (#457).
- Improve Russian translation (#408).
- Improve Swedish title (#419).
- Improve zh-CN translation (#359).
- Improve French translation (#357).
- Improve zh-TW translation (#360, #355).
- Improve Spanish (es-ES) transltion (#362).
- Foldout menu in Dutch translation (#371).
- Missing periods at the end of each change (#451).
- Fix missing logo in 1.1 pages
- Display notice when translation isn't for most recent version
- Various broken links, page versions, and indentations.

### Changed

- Upgrade dependencies: Ruby 3.2.1, Middleman, etc.

### Removed

- Unused normalize.css file
- Identical links assigned in each translation file
- Duplicate index file for the english version"#
        );
    }

    #[test]
    fn test_release_entry_parsing_with_alternate_date_format() {
        let changelog = Changelog::try_from(
            "## [Unreleased]\n\n## [1.0.10] 2023/05/10\n- Upgrade libcnb to 0.12.0",
        )
        .unwrap();
        let release_entry = changelog.releases.get("1.0.10").unwrap();
        assert_eq!(release_entry.version, "1.0.10".parse::<Version>().unwrap());
        assert_eq!(
            release_entry.date,
            Utc.with_ymd_and_hms(2023, 5, 10, 0, 0, 0).unwrap()
        );
        assert_eq!(release_entry.body, "- Upgrade libcnb to 0.12.0");
    }

    #[test]
    fn test_keep_a_changelog_parses_all_release_entries() {
        let changelog = Changelog::try_from(KEEP_A_CHANGELOG_1_0_0).unwrap();
        let releases = changelog.releases.keys().collect::<Vec<_>>();
        assert_eq!(
            releases,
            vec![
                "1.1.1", "1.1.0", "1.0.0", "0.3.0", "0.2.0", "0.1.0", "0.0.8", "0.0.7", "0.0.6",
                "0.0.5", "0.0.4", "0.0.3", "0.0.2", "0.0.1",
            ]
        );
    }

    #[test]
    fn test_keep_a_changelog_to_string() {
        let changelog = Changelog::try_from(KEEP_A_CHANGELOG_1_0_0).unwrap();
        assert_eq!(changelog.to_string(), KEEP_A_CHANGELOG_1_0_0);
    }

    #[test]
    fn test_generate_release_declarations() {
        let changelog = Changelog::try_from(KEEP_A_CHANGELOG_1_0_0).unwrap();
        let declarations = generate_release_declarations(
            &changelog,
            "https://github.com/olivierlacan/keep-a-changelog",
            &None,
        );
        assert_eq!(
            declarations,
            r#"[unreleased]: https://github.com/olivierlacan/keep-a-changelog/compare/v1.1.1...HEAD
[1.1.1]: https://github.com/olivierlacan/keep-a-changelog/compare/v1.1.0...v1.1.1
[1.1.0]: https://github.com/olivierlacan/keep-a-changelog/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/olivierlacan/keep-a-changelog/compare/v0.3.0...v1.0.0
[0.3.0]: https://github.com/olivierlacan/keep-a-changelog/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/olivierlacan/keep-a-changelog/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/olivierlacan/keep-a-changelog/compare/v0.0.8...v0.1.0
[0.0.8]: https://github.com/olivierlacan/keep-a-changelog/compare/v0.0.7...v0.0.8
[0.0.7]: https://github.com/olivierlacan/keep-a-changelog/compare/v0.0.6...v0.0.7
[0.0.6]: https://github.com/olivierlacan/keep-a-changelog/compare/v0.0.5...v0.0.6
[0.0.5]: https://github.com/olivierlacan/keep-a-changelog/compare/v0.0.4...v0.0.5
[0.0.4]: https://github.com/olivierlacan/keep-a-changelog/compare/v0.0.3...v0.0.4
[0.0.3]: https://github.com/olivierlacan/keep-a-changelog/compare/v0.0.2...v0.0.3
[0.0.2]: https://github.com/olivierlacan/keep-a-changelog/compare/v0.0.1...v0.0.2
[0.0.1]: https://github.com/olivierlacan/keep-a-changelog/releases/tag/v0.0.1"#
        );
    }

    #[test]
    fn test_generate_release_declarations_with_no_releases() {
        let changelog = Changelog::try_from("[Unreleased]").unwrap();
        let declarations = generate_release_declarations(
            &changelog,
            "https://github.com/olivierlacan/keep-a-changelog",
            &None,
        );
        assert_eq!(
            declarations,
            "[unreleased]: https://github.com/olivierlacan/keep-a-changelog"
        );
    }

    #[test]
    fn test_generate_release_declarations_with_only_one_release() {
        let changelog =
            Changelog::try_from("[Unreleased]\n## [0.0.1] - 2023-03-05\n\n- Some change\n")
                .unwrap();
        let declarations = generate_release_declarations(
            &changelog,
            "https://github.com/olivierlacan/keep-a-changelog",
            &None,
        );
        assert_eq!(
            declarations,
            "[unreleased]: https://github.com/olivierlacan/keep-a-changelog/compare/v0.0.1...HEAD\n[0.0.1]: https://github.com/olivierlacan/keep-a-changelog/releases/tag/v0.0.1"
        );
    }

    #[test]
    fn test_generate_release_declarations_starting_with_release() {
        let changelog = Changelog::try_from(KEEP_A_CHANGELOG_1_0_0).unwrap();
        let declarations = generate_release_declarations(
            &changelog,
            "https://github.com/olivierlacan/keep-a-changelog",
            &Some(Version {
                major: 1,
                minor: 0,
                patch: 0,
                pre: Default::default(),
                build: Default::default(),
            }),
        );
        assert_eq!(
            declarations,
            r#"[unreleased]: https://github.com/olivierlacan/keep-a-changelog/compare/v1.1.1...HEAD
[1.1.1]: https://github.com/olivierlacan/keep-a-changelog/compare/v1.1.0...v1.1.1
[1.1.0]: https://github.com/olivierlacan/keep-a-changelog/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/olivierlacan/keep-a-changelog/releases/tag/v1.0.0"#
        );
    }

    const KEEP_A_CHANGELOG_1_0_0: &str = r#"# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.1.1] - 2023-03-05

### Added

- Arabic translation (#444).
- v1.1 French translation.
- v1.1 Dutch translation (#371).
- v1.1 Russian translation (#410).
- v1.1 Japanese translation (#363).
- v1.1 Norwegian Bokmål translation (#383).
- v1.1 "Inconsistent Changes" Turkish translation (#347).
- Default to most recent versions available for each languages
- Display count of available translations (26 to date!)
- Centralize all links into `/data/links.json` so they can be updated easily

### Fixed

- Improve French translation (#377).
- Improve id-ID translation (#416).
- Improve Persian translation (#457).
- Improve Russian translation (#408).
- Improve Swedish title (#419).
- Improve zh-CN translation (#359).
- Improve French translation (#357).
- Improve zh-TW translation (#360, #355).
- Improve Spanish (es-ES) transltion (#362).
- Foldout menu in Dutch translation (#371).
- Missing periods at the end of each change (#451).
- Fix missing logo in 1.1 pages
- Display notice when translation isn't for most recent version
- Various broken links, page versions, and indentations.

### Changed

- Upgrade dependencies: Ruby 3.2.1, Middleman, etc.

### Removed

- Unused normalize.css file
- Identical links assigned in each translation file
- Duplicate index file for the english version

## [1.1.0] - 2019-02-15

### Added

- Danish translation (#297).
- Georgian translation from (#337).
- Changelog inconsistency section in Bad Practices.

### Fixed

- Italian translation (#332).
- Indonesian translation (#336).

## [1.0.0] - 2017-06-20

### Added

- New visual identity by [@tylerfortune8](https://github.com/tylerfortune8).
- Version navigation.
- Links to latest released version in previous versions.
- "Why keep a changelog?" section.
- "Who needs a changelog?" section.
- "How do I make a changelog?" section.
- "Frequently Asked Questions" section.
- New "Guiding Principles" sub-section to "How do I make a changelog?".
- Simplified and Traditional Chinese translations from [@tianshuo](https://github.com/tianshuo).
- German translation from [@mpbzh](https://github.com/mpbzh) & [@Art4](https://github.com/Art4).
- Italian translation from [@azkidenz](https://github.com/azkidenz).
- Swedish translation from [@magol](https://github.com/magol).
- Turkish translation from [@emreerkan](https://github.com/emreerkan).
- French translation from [@zapashcanon](https://github.com/zapashcanon).
- Brazilian Portuguese translation from [@Webysther](https://github.com/Webysther).
- Polish translation from [@amielucha](https://github.com/amielucha) & [@m-aciek](https://github.com/m-aciek).
- Russian translation from [@aishek](https://github.com/aishek).
- Czech translation from [@h4vry](https://github.com/h4vry).
- Slovak translation from [@jkostolansky](https://github.com/jkostolansky).
- Korean translation from [@pierceh89](https://github.com/pierceh89).
- Croatian translation from [@porx](https://github.com/porx).
- Persian translation from [@Hameds](https://github.com/Hameds).
- Ukrainian translation from [@osadchyi-s](https://github.com/osadchyi-s).

### Changed

- Start using "changelog" over "change log" since it's the common usage.
- Start versioning based on the current English version at 0.3.0 to help
  translation authors keep things up-to-date.
- Rewrite "What makes unicorns cry?" section.
- Rewrite "Ignoring Deprecations" sub-section to clarify the ideal
  scenario.
- Improve "Commit log diffs" sub-section to further argument against
  them.
- Merge "Why can’t people just use a git log diff?" with "Commit log
  diffs".
- Fix typos in Simplified Chinese and Traditional Chinese translations.
- Fix typos in Brazilian Portuguese translation.
- Fix typos in Turkish translation.
- Fix typos in Czech translation.
- Fix typos in Swedish translation.
- Improve phrasing in French translation.
- Fix phrasing and spelling in German translation.

### Removed

- Section about "changelog" vs "CHANGELOG".

## [0.3.0] - 2015-12-03

### Added

- RU translation from [@aishek](https://github.com/aishek).
- pt-BR translation from [@tallesl](https://github.com/tallesl).
- es-ES translation from [@ZeliosAriex](https://github.com/ZeliosAriex).

## [0.2.0] - 2015-10-06

### Changed

- Remove exclusionary mentions of "open source" since this project can
  benefit both "open" and "closed" source projects equally.

## [0.1.0] - 2015-10-06

### Added

- Answer "Should you ever rewrite a change log?".

### Changed

- Improve argument against commit logs.
- Start following [SemVer](https://semver.org) properly.

## [0.0.8] - 2015-02-17

### Changed

- Update year to match in every README example.
- Reluctantly stop making fun of Brits only, since most of the world
  writes dates in a strange way.

### Fixed

- Fix typos in recent README changes.
- Update outdated unreleased diff link.

## [0.0.7] - 2015-02-16

### Added

- Link, and make it obvious that date format is ISO 8601.

### Changed

- Clarified the section on "Is there a standard change log format?".

### Fixed

- Fix Markdown links to tag comparison URL with footnote-style links.

## [0.0.6] - 2014-12-12

### Added

- README section on "yanked" releases.

## [0.0.5] - 2014-08-09

### Added

- Markdown links to version tags on release headings.
- Unreleased section to gather unreleased changes and encourage note
  keeping prior to releases.

## [0.0.4] - 2014-08-09

### Added

- Better explanation of the difference between the file ("CHANGELOG")
  and its function "the change log".

### Changed

- Refer to a "change log" instead of a "CHANGELOG" throughout the site
  to differentiate between the file and the purpose of the file — the
  logging of changes.

### Removed

- Remove empty sections from CHANGELOG, they occupy too much space and
  create too much noise in the file. People will have to assume that the
  missing sections were intentionally left out because they contained no
  notable changes.

## [0.0.3] - 2014-08-09

### Added

- "Why should I care?" section mentioning The Changelog podcast.

## [0.0.2] - 2014-07-10

### Added

- Explanation of the recommended reverse chronological release ordering.

## [0.0.1] - 2014-05-31

### Added

- This CHANGELOG file to hopefully serve as an evolving example of a
  standardized open source project CHANGELOG.
- CNAME file to enable GitHub Pages custom domain.
- README now contains answers to common questions about CHANGELOGs.
- Good examples and basic guidelines, including proper date formatting.
- Counter-examples: "What makes unicorns cry?".
"#;
}
