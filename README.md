# Languages GitHub Actions

A set of custom GitHub Actions and reusable workflow used by the Languages Team.

- [Workflows](#workflows)
  - [Buildpacks - Prepare Release](#buildpacks---prepare-release)
  - [Buildpacks - Release](#buildpacks---release)
- [Actions](#actions)
  - [Install Languages CLI](#install-languages-cli)
- [Development](#development)

## Workflows

### Buildpacks - Prepare Release

Prepares a buildpack release by:
- bumping the fixed version
- updating changelogs
- generating an aggregate changelog from all the changelogs
- opening a PR against the repository with the modified files

You can pin to:
- the [latest release](https://github.com/heroku/languages-github-actions/releases/latest) version with `@latest`
- a [specific release](https://github.com/heroku/languages-github-actions/releases) version with `@v{major}.{minor}.{patch}`
- the development version with `@main`

#### Example Usage

```yaml
name: Prepare Buildpack Releases

on:
  workflow_dispatch:
    inputs:
      bump:
        description: "Bump"
        required: true
        default: 'patch'
        type: choice
        options:
          - major
          - minor
          - patch

jobs:
  prepare-release:
    uses: heroku/languages-github-actions/.github/workflows/_buildpacks-prepare-release.yml@latest
    with:
      app_id: ${{ vars.GH_APP_ID }}
      bump: ${{ inputs.bump }}
    secrets:
      app_private_key: ${{ secrets.GH_APP_PRIVATE_KEY }}

```

#### Inputs

| Name                            | Description                                                                                                                                                                                             | Required | Default                     |
|---------------------------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|----------|-----------------------------|
| `app_id`                        | Application ID of GitHub application (e.g. the Linguist App)                                                                                                                                            | true     |                             |
| `bump`                          | Bump which coordinate? (major, minor, patch)                                                                                                                                                            | true     |                             |
| `declarations_starting_version` | Only needed if existing releases have been published but there is no matching release tag in Git. If this is the case, the first git tag that matches a version from your CHANGELOG should be supplied. | false    |                             |
| `ip_allowlisted_runner`         | The GitHub Actions runner to use to run jobs that require IP allow-list privileges                                                                                                                      | false    | `pub-hk-ubuntu-22.04-small` |

#### Secrets

| Name              | Description                                  | Required |
|-------------------|----------------------------------------------|----------|
| `app_private_key` | Private key of GitHub application (Linguist) | true     |

### Buildpacks - Release

Prepares one or more buildpacks release by:

* Detects all the buildpacks in a repository and compiles them into Cloud Native Buildpacks
* For each compiled buildpack:
  * Creates a CNB archive file from the compiled buildpack and publishes it as a GitHub Release
  * Creates an OCI image from the compiled buildpack and publishes it to Docker
  * Retrieves the OCI image published to Docker Hub and registers this with the CNB Registry
* Once all buildpacks have been published, updates all the buildpack references found in [heroku/builder](https://github.com/heroku/builder)
  for the given list of builders and opens a pull requests containing all the changes to be committed.

You can pin to:
- the [latest release](https://github.com/heroku/languages-github-actions/releases/latest) version with `@latest`
- a [specific release](https://github.com/heroku/languages-github-actions/releases) version with `@v{major}.{minor}.{patch}`
- the development version with `@main`

#### Example Usage

```yaml
name: Release Buildpacks

on:
  workflow_dispatch:

jobs:
  release:
    name: Release
    uses: heroku/languages-github-actions/.github/workflows/_buildpacks-release.yml@latest
    with:
      app_id: ${{ vars.GH_APP_ID }}
    secrets:
      app_private_key: ${{ secrets.GH_APP_PRIVATE_KEY }}
      cnb_registry_token: ${{ secrets.CNB_REGISTRY_TOKEN }}
      docker_hub_user: ${{ secrets.DOCKER_HUB_USER }}
      docker_hub_token: ${{ secrets.DOCKER_HUB_TOKEN }}

```

#### Inputs

| Name                    | Description                                                                                         | Required | Default                     |
|-------------------------|-----------------------------------------------------------------------------------------------------|----------|-----------------------------|
| `app_id`                | Application ID of GitHub application (e.g. the Linguist App)                                        | true     |                             |
| `dry_run`               | Flag used for testing purposes to prevent actions that perform publishing operations from executing | false    | false                       |
| `ip_allowlisted_runner` | The GitHub Actions runner to use to run jobs that require IP allow-list privileges                  | false    | `pub-hk-ubuntu-22.04-small` |

#### Secrets

| Name                 | Description                                                         | Required |
|----------------------|---------------------------------------------------------------------|----------|
| `app_private_key`    | Private key of GitHub application (e.g. the Linguist App)           | true     |
| `cnb_registry_token` | The token of the GitHub user used to interact with the CNB registry | true     |
| `docker_hub_user`    | The username to login to Docker Hub with                            | true     |
| `docker_hub_token`   | The token to login to Docker Hub with                               | true     |

## Actions

### Install Languages CLI

Downloads the Languages CLI from a known release or installs using Cargo

#### Usage

```yaml
- name: Install Languages CLI
  uses: heroku/languages-github-actions/.github/actions/install-languages-cli@latest
```

You can pin to:
- the [latest release](https://github.com/heroku/languages-github-actions/releases/latest) version with `@latest`
- a [specific release](https://github.com/heroku/languages-github-actions/releases) version with `@v{major}.{minor}.{patch}`
- the development version with `@main`

#### Inputs

| Name                    | Description                                              | Required | Default |
|-------------------------|----------------------------------------------------------|----------|---------|
| `download_url`          | The url to download the CLI binary from                  | false    |         |
| `update_rust_toolchain` | Run `rustup update` before installing the CLI from Cargo | false    | true    |

## Development

Custom actions are written in [Rust](https://www.rust-lang.org/) and compiled into a command-line application that
exposes the following sub-commands:

```
Usage: actions <COMMAND>

Commands:
  generate-buildpack-matrix  Generates a JSON list of buildpack information for each buildpack detected
  generate-changelog         Generates a changelog from one or more buildpacks in a project
  prepare-release            Bumps the version of each detected buildpack and adds an entry for any unreleased changes from the changelog
  update-builder             Updates all references to a buildpack in heroku/builder for the given list of builders
  help                       Print this message or the help of the given subcommand(s)
```
