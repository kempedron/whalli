# Whalli Language Support

Official Visual Studio Code & VSCodium language support for the **Whalli** programming language.

## Features

- **Semantic Syntax Highlighting**: Full syntax highlighting for keywords, types, functions, variables, parameters, and comments.
- **Diagnostics**: Real-time syntax and lexer error reporting as you type.
- **Hover Documentation**: Rich Markdown documentation with signatures, parameter details, and copy-pasteable code examples for all standard library modules (`net`, `fs`, `time`, `math`), built-ins, and user functions.
- **IntelliSense & Auto-completion**: Context-aware autocompletion for modules, collection methods, language keywords, snippets (`func`, `struct`, `impl`, `interface`, `for in`, `range`), and user symbols.
- **Signature Help**: Parameter information and active parameter highlighting when calling functions.
- **Go to Definition**: Jump directly to definitions of functions, structs, interfaces, methods, and variables (`F12`).
- **Document Symbols & Outline**: Hierarchical symbols outline in the Explorer panel (`Ctrl+Shift+O`).
- **Code Formatting**: Automatic indentation and formatting (`Shift+Alt+F`).
- **Inlay Hints**: Inline type annotations for inferred variable types.
- **Rename Symbol**: Safe project-wide symbol renaming (`F2`).
- **Code Folding**: Fold blocks and control structures.

## Installation & Requirements

The extension uses the `whalli-lsp` language server binary.

1. Install `whalli` / `whalli-lsp` into your system `$PATH` (e.g. via `cargo install --path .` from the Whalli repository), or:
2. Configure the explicit binary path in VS Code Settings:
   ```json
   "whalli.lsp.serverPath": "/path/to/whalli-lsp"
   ```

## License

MIT
