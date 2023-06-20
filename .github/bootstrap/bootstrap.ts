/**
 * IMPORTANT! If you change this file be sure to regenerate the compiled version with `npm run build`
 */

"use strict";

import { readFileSync } from "node:fs"
import { join } from "node:path"
import { parse as urlParse } from "node:url"

import { setFailed, getInput, getBooleanInput, getMultilineInput, startGroup, endGroup, info } from "@actions/core"
import { exec } from "@actions/exec"
import { find, cacheFile, downloadTool, extractTar } from "@actions/tool-cache"
import { parse as tomlParse } from 'toml'

type GetArguments = (inputs: {
    getInput: typeof getInput,
    getBooleanInput: typeof getBooleanInput,
    getMultilineInput: typeof getMultilineInput
}) => string[]

export function invokeWith(getArgs: GetArguments) {
    executeRustBinaryAction(getArgs).catch(e => {
        if (e instanceof Error) {
            setFailed(e.message)
        }
    })
}

async function executeRustBinaryAction(getArgs: GetArguments) {
    startGroup('Bootstrapping');

    const { platform, env } = process
    const tempDirectory = env.RUNNER_TEMP

    if (platform !== 'win32' && platform !== 'darwin' && platform !== 'linux') {
        throw new Error(`Unsupported platform: ${platform}`)
    }

    const toml = tomlParse(readFileSync(join(__dirname, "../../Cargo.toml"), 'utf-8'))

    const { name } = toml.bin[0]
    info(`name: ${name}`)

    const { repository, version } = toml.package
    info(`version: ${version}\nrepository: ${repository}`)

    const binaryName = platform === 'win32' ? `${name}.exe` : name;
    info(`binaryName: ${binaryName}`)

    const githubOrgAndName = urlParse(repository).pathname
        .replace(/^\//, '')
        .replace(/\.git$/, '')

    // now we should be able to build up our download url which looks something like this:
    // https://github.com/heroku/languages-github-actions/releases/download/v0.0.0/actions-v0.0.0-darwin-x64.tar.gz
    const releaseUrl = `https://github.com/${githubOrgAndName}/releases/download/v${version}/${name}-v${version}-${platform}-x64.tar.gz`;
    info(`releaseUrl: ${releaseUrl}`)

    let cachedPath = find(githubOrgAndName, version)
    info(`is cached: ${cachedPath ? true : false}`)
    if (!cachedPath) {
        const downloadPath = await downloadTool(releaseUrl)
        info(`downloadPath: ${downloadPath}`)

        const extractPath = await extractTar(downloadPath, tempDirectory)
        info(`extractPath: ${extractPath}`)

        const extractedFile = join(extractPath, binaryName)
        info(`extractedFile: ${extractedFile}`)

        cachedPath = await cacheFile(extractedFile, binaryName, githubOrgAndName, version)
    }
    info(`using cache path: ${cachedPath}`)

    const rustBinary = join(cachedPath, name);
    info(`using binary: ${rustBinary}`)

    const args = getArgs({ getInput, getBooleanInput, getMultilineInput });
    info(`using args: ${args.join(" ")}`)

    endGroup()

    await exec(rustBinary, args);
}
