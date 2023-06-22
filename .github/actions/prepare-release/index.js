require('../../bootstrap').invokeWith(({ getInput }) => {
    const args = [
        'prepare-release',

        '--bump',
        getInput('bump', { required: true }),

        '--repository-url',
        getInput('repository_url')
    ]

    const declarationsStartingVersion = getInput('declarations_starting_version')
    if (declarationsStartingVersion) {
        args.push('--declarations-starting-version')
        args.push(declarationsStartingVersion)
    }

    return args
})
