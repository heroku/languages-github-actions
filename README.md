# Languages GitHub Actions

A set of custom GitHub Actions and reusable workflow used by the Languages Team.

- [Workflows](#workflows)
  - [Required Configuration](#required-configuration)
  - [Buildpacks - Prepare Release](#buildpacks---prepare-release)
  - [Buildpacks - Release](#buildpacks---release)
  - [Classic Buildpack - Prepare Release](#classic-buildpack---prepare-release)
  - [Classic Buildpack - Publish](#classic-buildpack---publish)
- [Actions](#actions)
  - [Install Languages CLI](#install-languages-cli)
- [Development](#development)

## Workflows

### Required Configuration

Before using the shared workflows below, please be sure to configure the following settings in your repository:

* [Edit the branch protection rules](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches/managing-a-branch-protection-rule#editing-a-branch-protection-rule)
  on the default branch (e.g. `main`). If **Restrict who can push to matching branches** and **Restrict pushes that create matching branches** is checked, then add the bot user
  of the GitHub Application that will be used to create pull requests and commits (e.g. `heroku-linguist`).
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

# Disable all GITHUB_TOKEN permissions, since the GitHub App token is used instead.
permissions: {}

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

| Name                            | Description                                                                                                                                                                                             | Required | Default                  |
|---------------------------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|----------|--------------------------|
| `app_id`                        | Application ID of GitHub application (e.g. the Linguist App)                                                                                                                                            | true     |                          |
| `bump`                          | Which component of the version to increment (major, minor, or patch)                                                                                                                                    | true     |                          |
| `declarations_starting_version` | Only needed if existing releases have been published but there is no matching release tag in Git. If this is the case, the first git tag that matches a version from your CHANGELOG should be supplied. | false    |                          |
| `ip_allowlisted_runner`         | The GitHub Actions runner to use to run jobs that require IP allow-list privileges                                                                                                                      | false    | `pub-hk-ubuntu-24.04-ip` |
| `languages_cli_branch`          | The branch to install the Languages CLI from (FOR TESTING)                                                                                                                                              | false    | `main`                   |

#### Outputs

| Name           | Description                                        |
|----------------|----------------------------------------------------|
| `from_version` | The version number of the existing latest release. |
| `to_version`   | The version number of the newly prepared release.  |

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
  # Auto-trigger when the "Prepare release" PR (created by the
  # _buildpacks-prepare-release.yml workflow) is merged.
  pull_request:
    branches:
      - main
    types:
      - closed
  workflow_dispatch:

# Disable all GITHUB_TOKEN permissions, since the GitHub App token is used instead.
permissions: {}

jobs:
  release:
    name: Release
    # On `pull_request`, only run for the merged auto-generated
    # "Prepare release" PR (branch name set by
    # _buildpacks-prepare-release.yml). Manual dispatches always run.
    if: >-
      github.event_name == 'workflow_dispatch' ||
      (github.event.pull_request.merged == true &&
       github.event.pull_request.head.repo.full_name == github.repository &&
       github.event.pull_request.head.ref == 'prepare-release' &&
       github.event.pull_request.user.login == 'heroku-linguist[bot]')
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

| Name                    | Description                                                                                         | Required | Default                  |
|-------------------------|-----------------------------------------------------------------------------------------------------|----------|--------------------------|
| `app_id`                | Application ID of GitHub application (e.g. the Linguist App)                                        | true     |                          |
| `dry_run`               | Flag used for testing purposes to prevent actions that perform publishing operations from executing | false    | false                    |
| `ip_allowlisted_runner` | The GitHub Actions runner to use to run jobs that require IP allow-list privileges                  | false    | `pub-hk-ubuntu-24.04-ip` |
| `languages_cli_branch`  | The branch to install the Languages CLI from (FOR TESTING)                                          | false    | `main`                   |

#### Secrets

| Name                 | Description                                                         | Required |
|----------------------|---------------------------------------------------------------------|----------|
| `app_private_key`    | Private key of GitHub application (e.g. the Linguist App)           | true     |
| `cnb_registry_token` | The token of the GitHub user used to interact with the CNB registry | true     |
| `docker_hub_user`    | The username to login to Docker Hub with                            | true     |
| `docker_hub_token`   | The token to login to Docker Hub with                               | true     |

### Classic Buildpack - Prepare Release

Prepares a "classic" buildpack release by:
- updating the changelog
- opening a PR against the repository with the modified files

You can pin to:
- the [latest release](https://github.com/heroku/languages-github-actions/releases/latest) version with `@latest`
- a [specific release](https://github.com/heroku/languages-github-actions/releases) version with `@v{major}.{minor}.{patch}`
- the development version with `@main`

#### Example Usage

```yaml
name: Prepare Buildpack Release

on:
  workflow_dispatch:

jobs:
  prepare-release:
    uses: heroku/languages-github-actions/.github/workflows/_classic-buildpack-prepare-release.yml@latest
    with:
      app_id: ${{ vars.LINGUIST_GH_APP_ID }}
    secrets:
      app_private_key: ${{ secrets.LINGUIST_GH_PRIVATE_KEY }}
```

#### Inputs

| Name                    | Description                                                                                                                    | Required | Default |
|-------------------------|--------------------------------------------------------------------------------------------------------------------------------|----------|---------|
| `app_id`                | Application ID of the GitHub application (e.g. the Linguist App) used to open the release PR                                   | true     |         |
| `custom_update_command` | An additional command to run before changes are committed, which can make use of the env vars EXISTING_VERSION and NEW_VERSION | false    |         |

In addition, the workflow requires that the `LINGUIST_GH_APP_USERNAME` and `LINGUIST_GH_APP_EMAIL` variables are available (which are set as organization variables).

#### Outputs

| Name           | Description                                        |
|----------------|----------------------------------------------------|
| `from_version` | The version number of the existing latest release. |
| `to_version`   | The version number of the newly prepared release.  |

#### Secrets

| Name              | Description                                                   | Required |
|-------------------|---------------------------------------------------------------|----------|
| `app_private_key` | Private key of the GitHub application (e.g. the Linguist App) | true     |

### Classic Buildpack - Publish

Publishes a "classic" buildpack release by:
- creating and pushing a `v{N}` git tag for the newest versioned heading in `CHANGELOG.md`
- publishing that tag to the buildpack registry (authenticated using GitHub OIDC) and waiting for the revision to be published
- creating a GitHub Release using the changelog entry as the release notes
- posting a release marker to Honeycomb (when `honeycomb_api_key` is provided)

The workflow is designed to be triggered by the merge of the pull request opened by [Classic Buildpack - Prepare Release](#classic-buildpack---prepare-release), or manually using `workflow_dispatch`. On a `pull_request` event it only runs once the pull request has been merged, and when not in QA mode the ref must be the default branch. The caller is responsible for restricting which pull requests count as the release PR.

Before publishing, a pre-flight check compares the existing tag, registry revision and GitHub Release for the version. Re-running after a partial failure resumes where it left off, but inconsistent state (for example a tag pointing at a different commit) fails the run so it can be cleaned up manually.

When a production publish started by a `pull_request` event begins, a comment is added to the prepare release PR. If it fails, a failure comment is added to that PR and a message is sent to Slack. Runs started with `workflow_dispatch` and QA runs do not send notifications.

Publishes for the same `buildpack_id` are serialized, so a second run waits for the first to finish rather than cancelling it.

You can pin to:
- the [latest release](https://github.com/heroku/languages-github-actions/releases/latest) version with `@latest`
- a [specific release](https://github.com/heroku/languages-github-actions/releases) version with `@v{major}.{minor}.{patch}`
- the development version with `@main`

#### Example Usage

```yaml
name: Release Buildpack

on:
  # Auto-trigger when the "Prepare release" PR (created by the
  # _classic-buildpack-prepare-release.yml workflow) is merged.
  pull_request:
    branches:
      - main
    types:
      - closed
  workflow_dispatch:

# The caller must grant at least the permissions the workflow needs.
permissions:
  id-token: write
  contents: write

jobs:
  release:
    name: Release
    # On `pull_request`, only run for the merged auto-generated
    # "Prepare release" PR. Manual dispatches always run.
    if: >-
      github.event_name == 'workflow_dispatch' ||
      (github.event.pull_request.merged == true &&
       github.event.pull_request.head.repo.full_name == github.repository &&
       github.event.pull_request.head.ref == 'prepare-release' &&
       github.event.pull_request.user.login == 'heroku-linguist[bot]')
    uses: heroku/languages-github-actions/.github/workflows/_classic-buildpack-publish.yml@latest
    with:
      buildpack_id: "heroku/ruby"
      app_id: ${{ vars.LINGUIST_GH_APP_ID }}
    secrets:
      app_private_key: ${{ secrets.LINGUIST_GH_PRIVATE_KEY }}
      slack_webhook_url: ${{ secrets.BUILDPACK_RELEASE_FAILURE_WEBHOOK_URL }}
      honeycomb_api_key: ${{ secrets.HONEYCOMB_API_KEY }}
```

#### Inputs

| Name           | Description                                                                                                               | Required | Default |
|----------------|---------------------------------------------------------------------------------------------------------------------------|----------|---------|
| `buildpack_id` | Registry name of the buildpack in `owner/name` form, using lowercase alphanumerics and hyphens (e.g. `heroku/go`)         | true     |         |
| `qa`           | QA mode: publishes to the staging registry, appends `-qa` to tags, and creates draft GitHub releases                      | false    | `false` |
| `app_id`       | Application ID of the GitHub application (e.g. the Linguist App) used to comment on the prepare PR and author the Release | true     |         |

#### Secrets

| Name                | Description                                                                                                                           | Required |
|---------------------|---------------------------------------------------------------------------------------------------------------------------------------|----------|
| `app_private_key`   | Private key of the GitHub application (e.g. the Linguist App)                                                                         | true     |
| `slack_webhook_url` | Slack Workflow Builder webhook URL for `#heroku-languages-ops`, used to post a message if a production publish fails                  | true     |
| `honeycomb_api_key` | Honeycomb API key with the "create markers" scope, used to post a release marker to the `builds` dataset after a production publish   | false    |

#### Permissions

The calling workflow must grant the following permissions, since the permissions of a reusable workflow are capped by what the caller provides:

| Name       | Level   | Used for                                                                    |
|------------|---------|-----------------------------------------------------------------------------|
| `id-token` | `write` | Minting the OIDC token the buildpack registry uses to authorize the publish |
| `contents` | `write` | Pushing the version tag and creating the GitHub Release                     |

In addition, the workflow runs on the `pub-hk-ubuntu-24.04-ip` runner group, which is required for the GitHub application token to be accepted by the IP allow list of the `heroku` organization.

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
