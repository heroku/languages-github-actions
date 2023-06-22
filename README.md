# Languages GitHub Actions

A set of custom GitHub Actions and reusable workflow used by the Languages Team.

## Actions

### Generate Buildpack Matrix

This action generates a list of buildpack `id` and `path` values.  E.g.;

```js
[
  {
    "id": "some/buildpack-id",
    "path": "/path/to/some/buildpack"
  },
  // ...
]
```

This list can be used in subsequent jobs with `jobs.<job_id>.strategy.matrix.include`
which accepts a list of key/value objects and will create a single job per buildpack.

See https://docs.github.com/en/actions/using-jobs/using-a-matrix-for-your-jobs#expanding-or-adding-matrix-configurations

#### Usage

```yaml
- name: Generate Buildpack Matrix
  uses: heroku/languages-github-actions/.github/actions/generate-buildpack-matrix@main
```

You can also pin to a [specific release](/releases) version in the format `@v{major}.{minor}.{patch}`

#### Outputs

| Name         | Description                                                     |
|--------------|-----------------------------------------------------------------|
| `buildpacks` | The list of buildpack (id, path) keys formatted as a JSON array |

### Generate Changelog

Generates an aggregated changelist from all buildpacks within a project.

#### Usage

```yaml
- name: Generate Changelog
  uses: heroku/languages-github-actions/.github/actions/generate-changelog@main
```

You can also pin to a [specific release](/releases) version in the format `@v{major}.{minor}.{patch}`

#### Inputs

| Name         | Description                                                      | Required | Default |
|--------------|------------------------------------------------------------------|----------|---------|
| `unreleased` | If the changelog should be generated from the unreleased section | false    |         |
| `version`    | If the changelog should be generated from a version section      | false    |         |

#### Outputs

| Name        | Description                          |
|-------------|--------------------------------------|
| `changelog` | Markdown content listing the changes |

### Prepare Release

Bumps the version of each detected buildpack and adds an entry for any unreleased changes from the changelog.

#### Usage

```yaml
- name: Prepare Buildpack Release
  uses: heroku/languages-github-actions/.github/actions/prepare-release@main
```

You can also pin to a [specific release](/releases) version in the format `@v{major}.{minor}.{patch}`

#### Inputs

| Name                            | Description                                                                              | Required | Default                                       |
|---------------------------------|------------------------------------------------------------------------------------------|----------|-----------------------------------------------|
| `bump`                          | Which coordinate should be incremented? (major, minor, patch)                            | true     |                                               |
| `repository_url`                | The URL of the repository (e.g.; https://github.com/octocat/Hello-World)                 | false    | `https://github.com/${{ github.repository }}` |
| `declarations_starting_version` | When generating markdown declarations for each release, what version should be the start | false    |                                               |

#### Outputs

| Name           | Description          |
|----------------|----------------------|
| `from_version` | The previous version |
| `to_version`   | The next version     |

### Update Builder

Updates all references to a buildpack in heroku/builder for the given list of builders.

#### Usage

```yaml
- name: Update Builder
  uses: heroku/languages-github-actions/.github/actions/update-builder@main
```

You can also pin to a [specific release](/releases) version in the format `@v{major}.{minor}.{patch}`

#### Inputs

| Name                | Description                                          | Required | Default            |
|---------------------|------------------------------------------------------|----------|--------------------|
| `buildpack_id`      | The id of the buildpack                              | true     |                    |
| `buildpack_version` | The version of the buildpack                         | true     |                    |
| `buildpack_uri`     | The URI of the published buildpack                   | true     |                    |
| `builders`          | A comma-separated list of builders to update         | true     |                    |
| `path`              | Relative path under `GITHUB_WORKSPACE` to execute in | false    | `GITHUB_WORKSPACE` |

## Development

Custom actions are written in [Rust](https://www.rust-lang.org/) and compiled into a command-line application that
exposes each action as a sub-command.

```shell
Usage: actions <COMMAND>

Commands:
  generate-buildpack-matrix  Generates a JSON list of {id, path} entries for each buildpack detected
  generate-changelog         Generates an aggregated changelist from all buildpacks within a project.
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

        '--path',
        getInput('path', { required: true }),

        '--buildpack-id',
        getInput('buildpack_id', { required: true }),

        '--buildpack-version',
        getInput('buildpack_version', { required: true }),

        '--buildpack-uri',
        getInput('buildpack_uri', { required: true }),

        '--builders',
        getInput('builders', { required: true }),
    ]
})
```
