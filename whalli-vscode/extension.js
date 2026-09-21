const { LanguageClient, TransportKind } = require('vscode-languageclient/node');

let client;

function activate(context) {
    const serverCommand = '/home/kepr/whalli/target/release/whalli-lsp'; 

    const serverOptions = {
        run: { command: serverCommand, transport: TransportKind.stdio },
        debug: { command: serverCommand, transport: TransportKind.stdio }
    };

    const clientOptions = {
        documentSelector: [{ scheme: 'file', language: 'whalli' }]
    };

    client = new LanguageClient('whalliLSP', 'Whalli Language Server', serverOptions, clientOptions);
    client.start();
}

function deactivate() {
    if (!client) return undefined;
    return client.stop();
}

module.exports = { activate, deactivate };