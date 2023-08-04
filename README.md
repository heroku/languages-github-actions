# Languages GitHub Actions

A set of custom GitHub Actions and reusable workflow used by the Languages Team.

- [Workflows](#workflows)
  - [Buildpacks - Prepare Release](#buildpacks---prepare-release)
- [Actions](#actions)
  - [Generate Buildpack Matrix](#generate-buildpack-matrix)
  - [Generate Changelog](#generate-changelog)
  - [Prepare Release](#prepare-release)
  - [Update Builder](#update-builder)
- [Development](#development)

## Workflows

### Buildpacks - Prepare Release

Prepares a buildpack release by:
* bumping the fixed version
* updating changelogs
* generating an aggregate changelog from all the changelogs
* opening a PR against the repository with the modified files

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
    uses: heroku/languages-github-actions/.github/workflows/_buildpacks-prepare-release.yml@main
    with:
      app_id: ${{ vars.GH_APP_ID }}
      bump: ${{ inputs.bump }}
    secrets:
      app_private_key: ${{ secrets.GH_APP_PRIVATE_KEY }}

```

#### Inputs

| Name                            | Description                                                                                                                                                                                             | Required | Default |
|---------------------------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|----------|---------|
| `bump`                          | Bump which coordinate? (major, minor, patch)                                                                                                                                                            | true     |         |
| `app_id`                        | Application ID of GitHub application (e.g. the Linguist App)                                                                                                                                                       | true     |         |
| `declarations_starting_version` | Only needed if existing releases have been published but there is no matching release tag in Git. If this is the case, the first git tag that matches a version from your CHANGELOG should be supplied. | false    |         |

#### Secrets

| Name              | Description                                  | Required |
|-------------------|----------------------------------------------|----------|
| `app_private_key` | Private key of GitHub application (Linguist) | true     |

### Buildpacks - Release

Prepares a buildpack release by:
* bumping the fixed version
* updating changelogs
* generating an aggregate changelog from all the changelogs
* opening a PR against the repository with the modified files

#### Example Usage

```yaml
name: Release Buildpacks

on:
  workflow_dispatch:

jobs:
  release:
    name: Release
    uses: heroku/languages-github-actions/.github/workflows/_buildpacks-release.yml@main
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

### Generate Buildpack Matrix

This action Generates a list of buildpack details for use in a matrix strategy `include` list. E.g.;

```js
[
  {
    "buildpack_id": "some/buildpack-id",
    "buildpack_version": "1.2.3",
    "buildpack_dir": "/path/to/some/buildpack",
    "buildpack_artifact_prefix": "some_buildpack-id",
    "docker_repository": "docker.io/some/repository"
  },
  // ...
]
```

This list can be used in subsequent jobs with `jobs.<job_id>.strategy.matrix.include`
which accepts a list of key/value objects and will create a single job for each
buildpack in the list.

See https://docs.github.com/en/actions/using-jobs/using-a-matrix-for-your-jobs#expanding-or-adding-matrix-configurations

#### Usage

```yaml
- name: Generate buildpack matrix
  uses: heroku/languages-github-actions/.github/actions/generate-buildpack-matrix@main
```

You can also pin to a [specific release](/releases) version in the format `@v{major}.{minor}.{patch}`

#### Outputs

| Name         | Description                                                                                                                                                 |
|--------------|-------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `buildpacks` | The list of {buildpack_id, buildpack_version, buildpack_dir, buildpack_artifact_prefix, docker_repository} values for a buildpack formatted as a JSON array |
| `version`    | The version number that all buildpacks are set to                                                                                                           |

### Generate Changelog

Generates a changelog from one or more buildpacks in a project.

#### Usage

```yaml
- name: Generate changelog
  uses: heroku/languages-github-actions/.github/actions/generate-changelog@main
```

You can also pin to a [specific release](/releases) version in the format `@v{major}.{minor}.{patch}`

#### Inputs

| Name         | Description                                                      | Required | Default            |
|--------------|------------------------------------------------------------------|----------|--------------------|
| `unreleased` | If the changelog should be generated from the unreleased section | false    |                    |
| `version`    | If the changelog should be generated from a version section      | false    |                    |
| `path`       | Relative path under `$GITHUB_WORKSPACE` to perform work in       | false    | `GITHUB_WORKSPACE` |

#### Outputs

| Name        | Description                          |
|-------------|--------------------------------------|
| `changelog` | Markdown content listing the changes |

### Prepare Release

Bumps the version of each detected buildpack and adds an entry for any unreleased changes from the changelog.

#### Usage

```yaml
- name: Prepare buildpack release
  uses: heroku/languages-github-actions/.github/actions/prepare-release@main
```

You can also pin to a [specific release](/releases) version in the format `@v{major}.{minor}.{patch}`

#### Inputs

| Name                            | Description                                                                               | Required | Default                                       |
|---------------------------------|-------------------------------------------------------------------------------------------|----------|-----------------------------------------------|
| `bump`                          | Which coordinate should be incremented? (major, minor, patch)                             | true     |                                               |
| `repository_url`                | The URL of the repository (e.g.; https://github.com/octocat/Hello-World)                  | false    | `https://github.com/${{ github.repository }}` |
| `declarations_starting_version` | When generating markdown declarations for each release, what version should be the start? | false    |                                               |

#### Outputs

| Name           | Description          |
|----------------|----------------------|
| `from_version` | The previous version |
| `to_version`   | The next version     |

### Update Builder

Updates all the buildpack references found in [`heroku/builder`](https://github.com/heroku/builder) for the given list of builders.

#### Usage

```yaml
- name: Update heroku/builder
  uses: heroku/languages-github-actions/.github/actions/update-builder@main
```

You can also pin to a [specific release](/releases) version in the format `@v{major}.{minor}.{patch}`

#### Inputs

| Name                      | Description                                                            | Required | Default |
|---------------------------|------------------------------------------------------------------------|----------|---------|
| `repository_path`         | Relative path under $GITHUB_WORKSPACE of the buildpack repository code | true     |         |
| `builder_repository_path` | Relative path under $GITHUB_WORKSPACE of the builder repository code   | true     |         |
| `builders`                | A list of builders to update                                           | true     |         |

## Development

Custom actions are written in [Rust](https://www.rust-lang.org/) and compiled into a command-line application that
exposes each action as a sub-command.

```
Usage: actions <COMMAND>

Commands:
  generate-buildpack-matrix  Generates a JSON list of buildpack information for each buildpack detected
  generate-changelog         Generates a changelog from one or more buildpacks in a project
  prepare-release            Bumps the version of each detected buildpack and adds an entry for any unreleased changes from the changelog
  update-builder             Updates all references to a buildpack in heroku/builder for the given list of builders
  help                       Print this message or the help of the given subcommand(s)
```

This `actions` command is bootstraped into the GitHub Action environment using the script found at
[`.github/bootstrap/bootstrap.ts`](.github/bootstrap/bootstrap.ts) which attempts to download this command from this
repository's [releases](/releases) page.

> **Note**
>
> Any changes made to this bootstrap script will need to be recompiled by running `npm run build` and committing the bundled
> script into GitHub. You'll need Node and NPM installed to do this.

Each of the custom actions must import this bootstrap script to obtain access to the `actions` command line application and
then it must provide a list of arguments to invoke the target action.

For example, this would invoke `actions generate-buildpack-matrix`:

```javascript
require('../../bootstrap').invokeWith(() => {
    return ['generate-buildpack-matrix']
})
```

And actions that declare inputs can forward those along to the command:

```javascript
require('../../bootstrap').invokeWith(({ getInput }) => {
    return [
        'update-builder',

        '--repository-path',
        getInput('repository_path', { required: true }),

        '--builder-repository-path',
        getInput('builder_repository_path', { required: true }),

        '--builders',
        getInput('builders', { required: true })
            .split('\n')
            .map(v => v.trim())
            .join(','),
    ]
})
```
