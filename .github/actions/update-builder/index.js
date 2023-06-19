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