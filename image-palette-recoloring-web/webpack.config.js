const path = require('path');

module.exports = {
    mode: 'development',
    entry: {
        main: './src/js/index.js',
        webworker: './src/js/webworker.js',
    },
    devtool: 'inline-source-map',
    output: {
        filename: '[name].js',
        path: path.resolve(__dirname, 'dist'),
    },
    experiments: {
      topLevelAwait: true,
    },
};

