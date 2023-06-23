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
