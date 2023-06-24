require('../../bootstrap').invokeWith(({ getInput }) => {
    const args = ['generate-changelog'];

    if (getInput('unreleased')) {
        args.push('--unreleased')
    } else if (getInput('version')) {
        args.push('--version')
        args.push(getInput('version'))
    }

    if (getInput('path')) {
        args.push('--path')
        args.push(getInput('path'))
    }

    return args
})
