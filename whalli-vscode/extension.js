const fs = require('fs');
const path = require('path');
const vscode = require('vscode');
const { LanguageClient, TransportKind } = require('vscode-languageclient/node');

let client;

function findServerPath(context) {
    const config = vscode.workspace.getConfiguration('whalli');
    const customPath = config.get('lsp.serverPath');
    if (customPath && customPath.trim() !== '') {
        if (fs.existsSync(customPath)) {
            return customPath;
        }
        vscode.window.showWarningMessage(`Whalli: Configured whalli-lsp path "${customPath}" does not exist.`);
    }

    const binaryName = process.platform === 'win32' ? 'whalli-lsp.exe' : 'whalli-lsp';

    const bundledPath = path.join(context.extensionPath, 'bin', binaryName);
    if (fs.existsSync(bundledPath)) {
        return bundledPath;
    }

    if (vscode.workspace.workspaceFolders) {
        for (const folder of vscode.workspace.workspaceFolders) {
            const relTarget = path.join(folder.uri.fsPath, 'target', 'release', binaryName);
            if (fs.existsSync(relTarget)) {
                return relTarget;
            }
            const dbgTarget = path.join(folder.uri.fsPath, 'target', 'debug', binaryName);
            if (fs.existsSync(dbgTarget)) {
                return dbgTarget;
            }
        }
    }

    const pathEnv = process.env.PATH || '';
    const pathDirs = pathEnv.split(path.delimiter);
    for (const dir of pathDirs) {
        const fullCandidate = path.join(dir, binaryName);
        if (fs.existsSync(fullCandidate)) {
            return fullCandidate;
        }
    }

    return binaryName;
}

function activate(context) {
    const serverCommand = findServerPath(context);

    const serverOptions = {
        run: { command: serverCommand, transport: TransportKind.stdio },
        debug: { command: serverCommand, transport: TransportKind.stdio }
    };

    const clientOptions = {
        documentSelector: [{ scheme: 'file', language: 'whalli' }]
    };

    client = new LanguageClient('whalliLSP', 'Whalli Language Server', serverOptions, clientOptions);

    client.start().catch((err) => {
        vscode.window.showErrorMessage(
            `Failed to start Whalli Language Server ("${serverCommand}"). ` +
            `Please ensure 'whalli-lsp' is installed in your PATH or configure 'whalli.lsp.serverPath' in Settings.`
        );
    });
}

function deactivate() {
    if (!client) return undefined;
    return client.stop();
}

module.exports = { activate, deactivate };
