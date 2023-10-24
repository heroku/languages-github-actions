# Languages GitHub Actions

A set of custom GitHub Actions and reusable workflow used by the Languages Team.

- [Workflows](#workflows)
  - [Required Configuration](#required-configuration)
  - [Buildpacks - Prepare Release](#buildpacks---prepare-release)
  - [Buildpacks - Release](#buildpacks---release)
- [Actions](#actions)
  - [Install Languages CLI](#install-languages-cli)
- [Development](#development)

## Workflows

### Required Configuration

Before using the shared workflows below, please be sure to configure the following settings in your repository:

* [Edit the branch protection rules](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches/managing-a-branch-protection-rule#editing-a-branch-protection-rule)
  on the default branch (e.g.; `main`). If **Restrict who can push to matching branches** and **Restrict pushes that create matching branches** is checked, then add the bot user
  of the GitHub Application that will be used to create pull requests and commits (e.g.; `heroku-linguist`).
* [Configure PR merges](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/configuring-pull-request-merges/managing-auto-merge-for-pull-requests-in-your-repository#managing-auto-merge)
  to **Allow auto-merge**.

> If either of these settings are misconfigured you will encounter errors during steps that
> create or configure pull requests. For example, an error message of **"Pull request User is
> not authorized for this protected branch"** indicates the branch protection rules are missing
> the GitHub Application bot user.

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
| `bump`                          | Which component of the version to increment (major, minor, or patch)                                                                                                                                    | true     |                             |
| `declarations_starting_version` | Only needed if existing releases have been published but there is no matching release tag in Git. If this is the case, the first git tag that matches a version from your CHANGELOG should be supplied. | false    |                             |
| `ip_allowlisted_runner`         | The GitHub Actions runner to use to run jobs that require IP allow-list privileges                                                                                                                      | false    | `pub-hk-ubuntu-22.04-small` |
| `languages_cli_branch`          | The branch to install the Languages CLI from (FOR TESTING)                                                                                                                                              | false    | `main`                      |

#### Secrets

| Name              | Description                                  | Required |
|-------------------|----------------------------------------------|----------|
| `app_private_key` | Private key of GitHub application (Linguist) | true     |

### Buildpacks - Release

Performs the release steps for one or more buildpacks by:

* Detecting all the buildpacks in a repository and compiling them into Cloud Native Buildpacks
* For each compiled buildpack:
  * Creating a CNB archive file from the compiled buildpack and publishing it as a GitHub Release
  * Creating an OCI image from the compiled buildpack and publishing it to the Docker Hub repository specified in the buildpack's `buildpack.toml`
    > The following metadata is used for declaring the registry:
    >
    > ```toml
    > [metadata.release]
    > image = { repository = "docker.io/heroku/buildpack-example" }
    > ```
  * Retrieving the OCI image url published to Docker Hub and registering this with the CNB Registry
* Once all buildpacks have been published, all the buildpack references found in [heroku/cnb-builder-images](https://github.com/heroku/cnb-builder-images)
  are updated for the given list of builders and a pull request is opened containing all the changes to be committed.

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
| `languages_cli_branch`  | The branch to install the Languages CLI from (FOR TESTING)                                          | false    | `main`                      |

#### Secrets

| Name                 | Description                                                         | Required |
|----------------------|---------------------------------------------------------------------|----------|
| `app_private_key`    | Private key of GitHub application (e.g. the Linguist App)           | true     |
| `cnb_registry_token` | The token of the GitHub user used to interact with the CNB registry | true     |
| `docker_hub_user`    | The username to login to Docker Hub with                            | true     |
| `docker_hub_token`   | The token to login to Docker Hub with                               | true     |

## Actions

### Install Languages CLI

Downloads the Languages CLI from a known release or installs using Cargo.

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
  update-builder             Updates all references to a buildpack in heroku/cnb-builder-images for the given list of builders
  help                       Print this message or the help of the given subcommand(s)
```
