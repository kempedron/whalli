# Tooling

> Source: `src/bin/whalli-lsp.rs`, `whalli-vscode/extension.js`, `README.md:198-219`.

## Language Server (`whalli-lsp`)

Binary: `whalli-lsp` (`Cargo.toml`, `src/bin/whalli-lsp.rs`).

- **Transport**: JSON-RPC over `stdio` via `tower-lsp 0.20` + `tokio` (`Cargo.toml:8-9`).
- **Capabilities**:
  - Diagnostics: syntax (`Lexer`/`Parser` errors) and semantic errors (undefined variables, import failures) pushed as you type.
  - Semantic tokens / highlighting.
  - Parameter hints.
  - Inlay hints for inferred variable types (`let x = 42` shows `: int`).
  - Go to definition (`F12`), Rename symbol (`F2`) — project-wide.
  - Document formatting (`Shift+Alt+F`) — auto-indentation based on braces.
  - Auto-complete: built-ins, stdlib modules (`net.`, `http.`, `json.`, `requests.`, `fs.`, `math.`, `time.`, `sync.`, `os.`), and user symbols.
  - Hover docs: signatures, parameters, code examples embedded in LSP (`whalli-lsp.rs:820-1646` snippet tables).
- **Invocation**: started automatically by the VS Code extension; manual: `whalli-lsp --stdio` (if exposed).

## VS Code / VSCodium Extension

Directory: `whalli-vscode/`, package `whalli-lang`.

- **Icon**: `whalli-vscode/icon.png`.
- **Features**:
  - One-click run: title bar button and `Ctrl+F5` executes current file with `whalli` binary path from settings (`whalli-vscode/extension.js:12`).
  - Syntax highlighting via TextMate grammar.
  - IDE integration: delegates to `whalli-lsp` for the above LSP capabilities.
- **Installation**:
  - Open VSX (VSCodium): search `whalli-lang`.
  - VS Code Marketplace: search `whalli-lang` or install `.vsix` bundle.
  - Setting key for custom binary path (if changed): configured via VS Code settings (see `extension.js`).

## CLI

- `whalli <file.wh>` — compile and run (VM). No separate compile step exposed.
- Future: `whalli fmt`, `whalli check` — not yet implemented (placeholders for book).

## Configuration

No `whalli.toml` at present. All tuning via environment: `WHALLI_NUM_THREADS`. GC threshold is adaptive, not user-configurable.

## Build Targets

- `cargo build --release` builds both `whalli` and `whalli-lsp` as separate `[[bin]]` targets.
