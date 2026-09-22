# Whalli Language Support

<p align="center">
  <img src="https://raw.githubusercontent.com/kempedron/whalli/main/whalli-vscode/icon.png" width="128" height="128" alt="Whalli Logo" />
</p>

Official Visual Studio Code & VSCodium language support for the **Whalli** programming language.

## Features

- **One-Click Run & Hotkey**: Click the **▶️ (Run)** button in the editor title bar or press **`Ctrl+F5`** to run your `.wh` script directly in an integrated Whalli terminal.
- **Semantic Syntax Highlighting**: Full syntax highlighting for keywords, types, functions, variables, parameters, and comments.
- **Diagnostics**: Real-time syntax and lexer error reporting as you type.
- **Hover Documentation with Examples**: Rich Markdown documentation with signatures, parameter details, and copy-pasteable code examples for all standard library modules (`net`, `fs`, `time`, `math`), built-ins, and user functions.
- **IntelliSense & Auto-completion**: Context-aware autocompletion for modules, collection methods, language keywords, snippets (`func`, `struct`, `impl`, `interface`, `for in`, `range`), and user symbols.
- **Signature Help**: Parameter information and active parameter highlighting when calling functions.
- **Go to Definition**: Jump directly to definitions of functions, structs, interfaces, methods, and variables (`F12`).
- **Document Symbols & Outline**: Hierarchical symbols outline in the Explorer panel (`Ctrl+Shift+O`).
- **Code Formatting**: Automatic indentation and formatting (`Shift+Alt+F`).
- **Inlay Hints**: Inline type annotations for inferred variable types.
- **Rename Symbol**: Safe project-wide symbol renaming (`F2`).
- **Code Folding**: Fold blocks and control structures.

## Installation & Configuration

1. **Language Server (`whalli-lsp`)**:
   - The extension automatically looks for `whalli-lsp` in the workspace `target/release`, `target/debug`, or your system `$PATH`.
   - You can also configure a custom path in Settings:
     ```json
     "whalli.lsp.serverPath": "/path/to/whalli-lsp"
     ```

2. **Interpreter (`whalli`)**:
   - For running scripts via `Ctrl+F5` / Run button, the extension automatically locates the `whalli` binary.
   - You can configure a custom interpreter path if needed:
     ```json
     "whalli.interpreterPath": "/path/to/whalli"
     ```

## License

MIT
