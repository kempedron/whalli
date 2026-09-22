const fs = require('fs');
const path = require('path');
const vscode = require('vscode');
const { LanguageClient, TransportKind } = require('vscode-languageclient/node');

let client;
let terminal;

function findBinaryPath(context, binaryBaseName, configKey) {
    // 1. Check user custom path from Settings
    const config = vscode.workspace.getConfiguration('whalli');
    const customPath = config.get(configKey);
    if (customPath && customPath.trim() !== '') {
        if (fs.existsSync(customPath)) {
            return customPath;
        }
        vscode.window.showWarningMessage(`Whalli: Configured path for "${binaryBaseName}" ("${customPath}") does not exist.`);
    }

    const binaryName = process.platform === 'win32' ? `${binaryBaseName}.exe` : binaryBaseName;

    // 2. Check bundled binary inside extension (e.g. <extension>/bin/...)
    const bundledPath = path.join(context.extensionPath, 'bin', binaryName);
    if (fs.existsSync(bundledPath)) {
        return bundledPath;
    }

    // 3. Check workspace target directory
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

    // 4. Check system PATH
    const pathEnv = process.env.PATH || '';
    const pathDirs = pathEnv.split(path.delimiter);
    for (const dir of pathDirs) {
        const fullCandidate = path.join(dir, binaryName);
        if (fs.existsSync(fullCandidate)) {
            return fullCandidate;
        }
    }

    // 5. Default fallback to executable name
    return binaryName;
}

function runActiveWhalliFile(context) {
    const editor = vscode.window.activeTextEditor;
    if (!editor) {
        vscode.window.showErrorMessage('Whalli: No active file to run.');
        return;
    }

    const document = editor.document;
    if (document.languageId !== 'whalli' && !document.fileName.endsWith('.wh')) {
        vscode.window.showErrorMessage('Whalli: Current file is not a Whalli file (.wh).');
        return;
    }

    // Save file before running if dirty
    if (document.isDirty) {
        document.save();
    }

    const interpreter = findBinaryPath(context, 'whalli', 'interpreterPath');
    const filePath = document.fileName;
    const fileDir = path.dirname(filePath);

    if (!terminal || terminal.exitStatus !== undefined) {
        terminal = vscode.window.createTerminal({
            name: 'Whalli',
            cwd: fileDir
        });
    }

    terminal.show(true);
    // Quote paths in case of spaces
    terminal.sendText(`"${interpreter}" "${filePath}"`);
}

function activate(context) {
    // 1. Register Run Command
    const runCommand = vscode.commands.registerCommand('whalli.runFile', () => {
        runActiveWhalliFile(context);
    });
    context.subscriptions.push(runCommand);

    // 2. Start LSP Client
    const serverCommand = findBinaryPath(context, 'whalli-lsp', 'lsp.serverPath');

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
