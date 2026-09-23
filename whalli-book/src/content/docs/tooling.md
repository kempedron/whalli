---
title: Tooling & IDE Support
description: Language Server Protocol (whalli-lsp) and VS Code extension features.
---

## Official VS Code Extension

Whalli includes full official IDE integration for **VS Code** and **VSCodium** located in `whalli-vscode/`.

### Key Capabilities

- **▶️ One-Click Run**: Instant script execution via the editor title bar or shortcut `Ctrl+F5`.
- **IntelliSense & Auto-Complete**: Context-aware completions for keywords, built-ins, standard library modules (`http.`, `sql.`, `net.`, `fs.`, `time.`, `math.`), and user definitions.
- **Hover Documentation**: Detailed type signatures, parameter descriptions, and executable usage snippets.
- **Real-Time Diagnostics**: Instant feedback on syntax errors and unknown module imports as you type.
- **Inlay Hints**: Inline type annotations for inferred variables (`let count = 42` shows `: int`).
- **Symbol Search & Go-To-Definition (`F12`)**: Fast jumping across files and declarations.
- **Document Formatting (`Shift+Alt+F`)**: Automatic code indentation and beautification.

## `whalli-lsp` Binary

The LSP server is built as a standalone binary communicating via standard JSON-RPC over `stdio`. It can be integrated with any editor supporting the Language Server Protocol (Neovim, Emacs, Helix, Sublime Text).
