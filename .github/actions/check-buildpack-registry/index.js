require('../../bootstrap').invokeWith(({ getInput }) => {
    return [
        'check-buildpack-registry',

        '--buildpack-id',
        getInput('buildpack_id', { required: true }),

        '--buildpack-version',
        getInput('buildpack_version', { required: true }),

        '--path',
        getInput('path', { required: true })
    ]
})
