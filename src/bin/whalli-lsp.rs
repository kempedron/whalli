use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use whalli::lexer::{Lexer, TokenKind};
use whalli::parser::Parser;

const LEGEND_TYPES: &[SemanticTokenType] = &[
    SemanticTokenType::KEYWORD,        // 0
    SemanticTokenType::TYPE,           // 1
    SemanticTokenType::FUNCTION,       // 2
    SemanticTokenType::VARIABLE,       // 3
    SemanticTokenType::PARAMETER,      // 4
    SemanticTokenType::PROPERTY,       // 5
    SemanticTokenType::STRING,         // 6
    SemanticTokenType::NUMBER,         // 7
    SemanticTokenType::OPERATOR,       // 8
    SemanticTokenType::COMMENT,        // 9
    SemanticTokenType::STRUCT,         // 10
    SemanticTokenType::INTERFACE,      // 11
];

const LEGEND_MODIFIERS: &[SemanticTokenModifier] = &[
    SemanticTokenModifier::DECLARATION,      // 1 << 0
    SemanticTokenModifier::DEFAULT_LIBRARY, // 1 << 1
];

#[derive(Debug, Clone)]
pub struct FnSymbol {
    pub name: String,
    pub params: Vec<(String, Option<String>)>,
    pub return_type: Option<String>,
    pub line: u32,
    pub col: u32,
    pub doc: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StructSymbol {
    pub name: String,
    pub fields: Vec<(String, String)>,
    pub line: u32,
    pub col: u32,
    pub doc: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ImplSymbol {
    pub struct_name: String,
    pub methods: Vec<FnSymbol>,
    pub line: u32,
    pub col: u32,
}

#[derive(Debug, Clone)]
pub struct InterfaceSymbol {
    pub name: String,
    pub methods: Vec<String>,
    pub line: u32,
    pub col: u32,
    pub doc: Option<String>,
}

#[derive(Debug, Clone)]
pub struct VarSymbol {
    pub name: String,
    pub line: u32,
    pub col: u32,
    pub is_param: bool,
    pub inferred_type: Option<String>,
}

#[derive(Default, Debug, Clone)]
pub struct DocumentIndex {
    pub functions: Vec<FnSymbol>,
    pub structs: Vec<StructSymbol>,
    pub impls: Vec<ImplSymbol>,
    pub interfaces: Vec<InterfaceSymbol>,
    pub variables: Vec<VarSymbol>,
}

pub struct Backend {
    client: Client,
    documents: Mutex<HashMap<Url, String>>,
}

impl Backend {
    fn get_line(&self, text: &str, line_idx: usize) -> String {
        text.lines().nth(line_idx).unwrap_or("").to_string()
    }

    async fn validate_document(&self, uri: Url, text: &str) {
        let mut diagnostics = Vec::new();
        let lines: Vec<&str> = text.lines().collect();

        let mut lexer = Lexer::new(text);
        match lexer.tokenize() {
            Ok(tokens) => {
                let mut lex_error_found = false;
                for token in &tokens {
                    if let TokenKind::Error(ref err_msg) = token.kind {
                        lex_error_found = true;
                        let line_idx = token.line.saturating_sub(1);
                        let line_len = lines.get(line_idx).map_or(0, |l| l.len());
                        let range = Range {
                            start: Position {
                                line: line_idx as u32,
                                character: 0,
                            },
                            end: Position {
                                line: line_idx as u32,
                                character: line_len as u32,
                            },
                        };
                        diagnostics.push(Diagnostic {
                            range,
                            severity: Some(DiagnosticSeverity::ERROR),
                            source: Some("whalli-lexer".to_string()),
                            message: err_msg.clone(),
                            ..Default::default()
                        });
                    }
                }

                if !lex_error_found {
                    let mut parser = Parser::new(tokens.clone());
                    if let Err(parse_err) = parser.parse() {
                        let line_idx = parse_err.line.saturating_sub(1);
                        let line_str = lines.get(line_idx).unwrap_or(&"");
                        let start_col = line_str
                            .chars()
                            .take_while(|c| c.is_whitespace())
                            .count();
                        let end_col = line_str.len().max(start_col + 1);

                        let range = Range {
                            start: Position {
                                line: line_idx as u32,
                                character: start_col as u32,
                            },
                            end: Position {
                                line: line_idx as u32,
                                character: end_col as u32,
                            },
                        };

                        diagnostics.push(Diagnostic {
                            range,
                            severity: Some(DiagnosticSeverity::ERROR),
                            source: Some("whalli-parser".to_string()),
                            message: format!("Syntax error: {}", parse_err.message),
                            ..Default::default()
                        });
                    }
                }

                // Check imports and static analysis
                let index = index_document(text);

                // 1. Module import check
                let mut i = 0;
                while i < tokens.len() {
                    if tokens[i].kind == TokenKind::Import && i + 1 < tokens.len() {
                        if let TokenKind::Identifier(ref mod_name) = tokens[i + 1].kind {
                            if !["net", "fs", "time", "math"].contains(&mod_name.as_str()) {
                                let line_idx = tokens[i].line.saturating_sub(1);
                                let line_str = lines.get(line_idx).unwrap_or(&"");
                                let start_col = line_str.find(mod_name.as_str()).unwrap_or(0);
                                diagnostics.push(Diagnostic {
                                    range: Range {
                                        start: Position {
                                            line: line_idx as u32,
                                            character: start_col as u32,
                                        },
                                        end: Position {
                                            line: line_idx as u32,
                                            character: (start_col + mod_name.len()) as u32,
                                        },
                                    },
                                    severity: Some(DiagnosticSeverity::WARNING),
                                    source: Some("whalli".to_string()),
                                    message: format!(
                                        "Unknown module '{}'. Built-in modules are: net, fs, time, math",
                                        mod_name
                                    ),
                                    ..Default::default()
                                });
                            }
                        }
                    }
                    i += 1;
                }

                // 2. Unused variables warning (dimmed with UNNECESSARY tag)
                for var in &index.variables {
                    if var.is_param || var.name.starts_with('_') {
                        continue;
                    }
                    let count = tokens
                        .iter()
                        .filter(|t| match &t.kind {
                            TokenKind::Identifier(name) => name == &var.name,
                            _ => false,
                        })
                        .count();

                    if count <= 1 {
                        diagnostics.push(Diagnostic {
                            range: Range {
                                start: Position {
                                    line: var.line,
                                    character: var.col,
                                },
                                end: Position {
                                    line: var.line,
                                    character: var.col + var.name.len() as u32,
                                },
                            },
                            severity: Some(DiagnosticSeverity::HINT),
                            tags: Some(vec![DiagnosticTag::UNNECESSARY]),
                            source: Some("whalli-lint".to_string()),
                            message: format!(
                                "Variable '{}' is declared but never read. Prefix with '_' if intentional.",
                                var.name
                            ),
                            ..Default::default()
                        });
                    }
                }
            }
            Err(lex_err) => {
                let line_idx = lex_err.line.saturating_sub(1);
                let line_len = lines.get(line_idx).map_or(0, |l| l.len());
                let range = Range {
                    start: Position {
                        line: line_idx as u32,
                        character: 0,
                    },
                    end: Position {
                        line: line_idx as u32,
                        character: line_len as u32,
                    },
                };
                diagnostics.push(Diagnostic {
                    range,
                    severity: Some(DiagnosticSeverity::ERROR),
                    source: Some("whalli-lexer".to_string()),
                    message: lex_err.message,
                    ..Default::default()
                });
            }
        }

        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![".".to_string(), ":".to_string(), " ".to_string()]),
                    ..Default::default()
                }),
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                signature_help_provider: Some(SignatureHelpOptions {
                    trigger_characters: Some(vec!["(".to_string(), ",".to_string()]),
                    retrigger_characters: None,
                    work_done_progress_options: Default::default(),
                }),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            work_done_progress_options: Default::default(),
                            legend: SemanticTokensLegend {
                                token_types: LEGEND_TYPES.to_vec(),
                                token_modifiers: LEGEND_MODIFIERS.to_vec(),
                            },
                            range: None,
                            full: Some(SemanticTokensFullOptions::Bool(true)),
                        },
                    ),
                ),
                document_formatting_provider: Some(OneOf::Left(true)),
                document_highlight_provider: Some(OneOf::Left(true)),
                rename_provider: Some(OneOf::Left(true)),
                folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
                inlay_hint_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Whalli Language Server started successfully!")
            .await;
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        {
            let mut docs = self.documents.lock().unwrap();
            docs.insert(uri.clone(), text.clone());
        }
        self.validate_document(uri, &text).await;
    }

    async fn did_change(&self, mut params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some(change) = params.content_changes.pop() {
            let text = change.text;
            {
                let mut docs = self.documents.lock().unwrap();
                docs.insert(uri.clone(), text.clone());
            }
            self.validate_document(uri, &text).await;
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = {
            let docs = self.documents.lock().unwrap();
            docs.get(&uri).cloned().unwrap_or_default()
        };
        if !text.is_empty() {
            self.validate_document(uri, &text).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        {
            let mut docs = self.documents.lock().unwrap();
            docs.remove(&uri);
        }
        self.client.publish_diagnostics(uri, vec![], None).await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let text = {
            let docs = self.documents.lock().unwrap();
            docs.get(&uri).cloned().unwrap_or_default()
        };

        if text.is_empty() {
            return Ok(None);
        }

        let index = index_document(&text);
        let hover_res = get_hover_info(&text, position, &index);
        Ok(hover_res)
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let text = {
            let docs = self.documents.lock().unwrap();
            docs.get(&uri).cloned().unwrap_or_default()
        };

        if text.is_empty() {
            return Ok(None);
        }

        let index = index_document(&text);
        let target_location = find_definition(&text, position, &uri, &index);
        Ok(target_location.map(GotoDefinitionResponse::Scalar))
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        let text = {
            let docs = self.documents.lock().unwrap();
            docs.get(&uri).cloned().unwrap_or_default()
        };

        if text.is_empty() {
            return Ok(None);
        }

        let index = index_document(&text);
        let symbols = build_document_symbols(&index);
        Ok(Some(DocumentSymbolResponse::Nested(symbols)))
    }

    async fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let text = {
            let docs = self.documents.lock().unwrap();
            docs.get(&uri).cloned().unwrap_or_default()
        };

        if text.is_empty() {
            return Ok(None);
        }

        let index = index_document(&text);
        let sig_help = get_signature_help(&text, position, &index);
        Ok(sig_help)
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri;
        let text = {
            let docs = self.documents.lock().unwrap();
            docs.get(&uri).cloned().unwrap_or_default()
        };

        if text.is_empty() {
            return Ok(None);
        }

        let tokens = compute_semantic_tokens(&text);
        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data: tokens,
        })))
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri;
        let text = {
            let docs = self.documents.lock().unwrap();
            docs.get(&uri).cloned().unwrap_or_default()
        };

        if text.is_empty() {
            return Ok(None);
        }

        let formatted = format_whalli_code(&text);
        if formatted == text {
            return Ok(None);
        }

        let line_count = text.lines().count();
        let last_line_len = text.lines().last().map_or(0, |l| l.len());
        let range = Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: line_count as u32,
                character: last_line_len as u32,
            },
        };

        Ok(Some(vec![TextEdit {
            range,
            new_text: formatted,
        }]))
    }

    async fn inlay_hint(&self, params: InlayHintParams) -> Result<Option<Vec<InlayHint>>> {
        let uri = params.text_document.uri;
        let text = {
            let docs = self.documents.lock().unwrap();
            docs.get(&uri).cloned().unwrap_or_default()
        };

        if text.is_empty() {
            return Ok(None);
        }

        let index = index_document(&text);
        let hints = compute_inlay_hints(&text, &index);
        Ok(Some(hints))
    }

    async fn document_highlight(
        &self,
        params: DocumentHighlightParams,
    ) -> Result<Option<Vec<DocumentHighlight>>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let text = {
            let docs = self.documents.lock().unwrap();
            docs.get(&uri).cloned().unwrap_or_default()
        };

        if text.is_empty() {
            return Ok(None);
        }

        let word_info = match get_word_at_position(&text, position) {
            Some(w) => w,
            None => return Ok(None),
        };

        let target = word_info.word;
        let mut highlights = Vec::new();

        for (line_idx, line) in text.lines().enumerate() {
            let mut start_col = 0;
            while let Some(idx) = line[start_col..].find(&target) {
                let actual_col = start_col + idx;
                let end_col = actual_col + target.len();

                let is_prev_boundary = actual_col == 0
                    || !line
                        .chars()
                        .nth(actual_col - 1)
                        .map_or(false, |c| c.is_alphanumeric() || c == '_');
                let is_next_boundary = end_col == line.len()
                    || !line
                        .chars()
                        .nth(end_col)
                        .map_or(false, |c| c.is_alphanumeric() || c == '_');

                if is_prev_boundary && is_next_boundary {
                    highlights.push(DocumentHighlight {
                        range: Range {
                            start: Position {
                                line: line_idx as u32,
                                character: actual_col as u32,
                            },
                            end: Position {
                                line: line_idx as u32,
                                character: end_col as u32,
                            },
                        },
                        kind: Some(DocumentHighlightKind::TEXT),
                    });
                }
                start_col = end_col;
            }
        }

        Ok(Some(highlights))
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let new_name = params.new_name;

        let text = {
            let docs = self.documents.lock().unwrap();
            docs.get(&uri).cloned().unwrap_or_default()
        };

        if text.is_empty() {
            return Ok(None);
        }

        let word_info = match get_word_at_position(&text, position) {
            Some(w) => w,
            None => return Ok(None),
        };

        let target = word_info.word;

        // Do not allow renaming builtins or keywords
        let reserved = [
            "net", "fs", "time", "math", "println", "print", "range", "new", "len",
            "push", "pop", "keys", "remove", "int", "float", "str", "bool", "let",
            "func", "return", "if", "else", "while", "for", "in", "break", "continue",
            "import", "struct", "impl", "is", "interface", "wo", "true", "false", "nil",
        ];
        if reserved.contains(&target.as_str()) {
            return Ok(None);
        }

        let mut edits = Vec::new();
        for (line_idx, line) in text.lines().enumerate() {
            let mut start_col = 0;
            while let Some(idx) = line[start_col..].find(&target) {
                let actual_col = start_col + idx;
                let end_col = actual_col + target.len();

                let is_prev_boundary = actual_col == 0
                    || !line
                        .chars()
                        .nth(actual_col - 1)
                        .map_or(false, |c| c.is_alphanumeric() || c == '_');
                let is_next_boundary = end_col == line.len()
                    || !line
                        .chars()
                        .nth(end_col)
                        .map_or(false, |c| c.is_alphanumeric() || c == '_');

                if is_prev_boundary && is_next_boundary {
                    edits.push(TextEdit {
                        range: Range {
                            start: Position {
                                line: line_idx as u32,
                                character: actual_col as u32,
                            },
                            end: Position {
                                line: line_idx as u32,
                                character: end_col as u32,
                            },
                        },
                        new_text: new_name.clone(),
                    });
                }
                start_col = end_col;
            }
        }

        let mut changes = HashMap::new();
        changes.insert(uri, edits);
        Ok(Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        }))
    }

    async fn folding_range(&self, params: FoldingRangeParams) -> Result<Option<Vec<FoldingRange>>> {
        let uri = params.text_document.uri;
        let text = {
            let docs = self.documents.lock().unwrap();
            docs.get(&uri).cloned().unwrap_or_default()
        };

        if text.is_empty() {
            return Ok(None);
        }

        let mut folding_ranges = Vec::new();
        let mut brace_stack = Vec::new();

        for (line_idx, line) in text.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") {
                continue;
            }

            let mut in_str = false;
            let mut escape = false;

            for c in line.chars() {
                if in_str {
                    if escape {
                        escape = false;
                    } else if c == '\\' {
                        escape = true;
                    } else if c == '"' {
                        in_str = false;
                    }
                } else if c == '"' {
                    in_str = true;
                } else if c == '{' {
                    brace_stack.push(line_idx as u32);
                } else if c == '}' {
                    if let Some(start_line) = brace_stack.pop() {
                        if (line_idx as u32) > start_line {
                            folding_ranges.push(FoldingRange {
                                start_line,
                                start_character: None,
                                end_line: line_idx as u32,
                                end_character: None,
                                kind: Some(FoldingRangeKind::Region),
                                collapsed_text: None,
                            });
                        }
                    }
                }
            }
        }

        Ok(Some(folding_ranges))
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        let text = {
            let docs = self.documents.lock().unwrap();
            docs.get(&uri).cloned().unwrap_or_default()
        };

        if text.is_empty() {
            return Ok(None);
        }

        let line = self.get_line(&text, position.line as usize);
        let char_idx = position.character as usize;

        let text_before_cursor = line.chars().take(char_idx).collect::<String>();
        let trimmed_before = text_before_cursor.trim_end();

        let is_dot_completion = trimmed_before.ends_with('.');
        let index = index_document(&text);

        let mut items = vec![];

        if is_dot_completion {
            let caller_name = trimmed_before
                .trim_end_matches('.')
                .split_whitespace()
                .last()
                .unwrap_or("");

            match caller_name {
                "net" => {
                    items.push(create_snippet("listen", "func net.listen(port: int) -> int\nStarts TCP server", "listen(${1:port})"));
                    items.push(create_snippet("accept", "func net.accept(server_id: int) -> int\nAccepts incoming connection", "accept(${1:server_id})"));
                    items.push(create_snippet("read", "func net.read(client_id: int) -> str\nReads data from client socket", "read(${1:client_id})"));
                    items.push(create_snippet("write", "func net.write(client_id: int, data: str) -> bool\nWrites data to client socket", "write(${1:client_id}, ${2:data})"));
                    items.push(create_snippet("close", "func net.close(client_id: int) -> bool\nCloses client socket", "close(${1:client_id})"));
                }
                "fs" => {
                    items.push(create_snippet("read", "func fs.read(path: str) -> str\nReads file contents into string", "read(${1:\"path\"})"));
                    items.push(create_snippet("write", "func fs.write(path: str, data: str) -> bool\nWrites data string to file", "write(${1:\"path\"}, ${2:data})"));
                }
                "time" => {
                    items.push(create_snippet("sleep", "func time.sleep(seconds: float)\nSuspends woroutine for specified seconds", "sleep(${1:seconds})"));
                    items.push(create_snippet("now", "func time.now() -> float\nReturns UNIX timestamp in seconds", "now()"));
                }
                "math" => {
                    items.push(CompletionItem {
                        label: "pi".to_string(),
                        kind: Some(CompletionItemKind::CONSTANT),
                        detail: Some("const math.pi: float = 3.141592653589793".to_string()),
                        documentation: Some(Documentation::MarkupContent(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: "The mathematical constant $\\pi$.".to_string(),
                        })),
                        ..Default::default()
                    });
                    items.push(create_snippet("sin", "func math.sin(rad: float) -> float\nSine function (radians)", "sin(${1:rad})"));
                }
                _ => {
                    // Check if caller matches a known struct with methods
                    for imp in &index.impls {
                        if imp.struct_name == caller_name {
                            for m in &imp.methods {
                                let snippet = if m.params.is_empty() {
                                    format!("{}()", m.name)
                                } else {
                                    let args: Vec<String> = m
                                        .params
                                        .iter()
                                        .enumerate()
                                        .map(|(idx, (p, _))| format!("${{{}:{}}}", idx + 1, p))
                                        .collect();
                                    format!("{}({})", m.name, args.join(", "))
                                };
                                items.push(create_snippet(
                                    &m.name,
                                    &format!("method of struct {}", imp.struct_name),
                                    &snippet,
                                ));
                            }
                        }
                    }

                    // Builtin methods on collections/strings
                    items.push(create_snippet("push", "method push(item: any)\nAppends item to list", "push(${1:item})"));
                    items.push(create_snippet("pop", "method pop() -> any\nRemoves and returns last item of list", "pop()"));
                    items.push(create_snippet("len", "method len() -> int\nReturns length of collection or string", "len()"));
                    items.push(create_snippet("keys", "method keys() -> list\nReturns list of map keys", "keys()"));
                    items.push(create_snippet("remove", "method remove(key: str) -> any\nRemoves key from map", "remove(${1:key})"));
                }
            }
        } else {
            // Builtin snippets
            items.push(create_snippet("println", "println(...args: any)\nPrints values to stdout with newline", "println(${1:value})"));
            items.push(create_snippet("print", "print(...args: any)\nPrints values to stdout without newline", "print(${1:value})"));
            items.push(create_snippet("range", "range(start, end, step = 1)\nGenerates numeric range for loop", "range(${1:start}, ${2:end}, ${3:step})"));
            items.push(create_snippet("new list", "new(\"list\")\nAllocates a new heap list", "new(\"list\")"));
            items.push(create_snippet("new map", "new(\"map\")\nAllocates a new heap map", "new(\"map\")"));
            items.push(create_snippet("new chan", "new(\"chan\", capacity)\nAllocates a new woroutine channel", "new(\"chan\", ${1:capacity})"));

            // Language construct snippets
            items.push(create_snippet("func", "Function declaration", "func ${1:name}(${2:params}) {\n\t${0}\n}"));
            items.push(create_snippet("func->", "Function with return type", "func ${1:name}(${2:params}) -> ${3:type} {\n\t${0}\n}"));
            items.push(create_snippet("struct", "Struct declaration", "struct ${1:Name} {\n\t${2:field}: ${3:type}\n}"));
            items.push(create_snippet("impl", "Method implementation block", "impl ${1:Name} {\n\tfunc ${2:method}(${3:params}) {\n\t\t${0}\n\t}\n}"));
            items.push(create_snippet("interface", "Interface declaration", "interface ${1:Name} {\n\t${2:method}\n}"));
            items.push(create_snippet("for in", "Iterate over collection", "for ${1:item} in ${2:collection} {\n\t${0}\n}"));
            items.push(create_snippet("for range", "Iterate over numeric range", "for ${1:i} in range(${2:0}, ${3:10}) {\n\t${0}\n}"));
            items.push(create_snippet("while", "While loop", "while ${1:condition} {\n\t${0}\n}"));
            items.push(create_snippet("if", "If condition", "if ${1:condition} {\n\t${0}\n}"));
            items.push(create_snippet("ifelse", "If-Else condition", "if ${1:condition} {\n\t${2}\n} else {\n\t${0}\n}"));
            items.push(create_snippet("wo", "Spawn async woroutine", "wo ${1:func_name}(${2})"));

            // Keywords
            for kw in [
                "let", "func", "return", "if", "else", "while", "for", "in",
                "and", "or", "not", "break", "continue", "import", "struct",
                "impl", "is", "interface", "wo", "true", "false", "nil",
            ] {
                items.push(CompletionItem {
                    label: kw.to_string(),
                    kind: Some(CompletionItemKind::KEYWORD),
                    ..Default::default()
                });
            }

            // Types
            for ty in ["int", "float", "str", "bool", "list", "map", "chan", "tuple"] {
                items.push(CompletionItem {
                    label: ty.to_string(),
                    kind: Some(CompletionItemKind::TYPE_PARAMETER),
                    detail: Some("Built-in Type".to_string()),
                    ..Default::default()
                });
            }

            // Modules
            for m in ["net", "fs", "time", "math"] {
                items.push(CompletionItem {
                    label: m.to_string(),
                    kind: Some(CompletionItemKind::MODULE),
                    detail: Some("Standard Library Module".to_string()),
                    ..Default::default()
                });
            }

            // User-defined functions
            let mut unique_names = HashSet::new();
            for f in &index.functions {
                if unique_names.insert(f.name.clone()) {
                    let snippet = if f.params.is_empty() {
                        format!("{}()", f.name)
                    } else {
                        let args: Vec<String> = f
                            .params
                            .iter()
                            .enumerate()
                            .map(|(idx, (p, _))| format!("${{{}:{}}}", idx + 1, p))
                            .collect();
                        format!("{}({})", f.name, args.join(", "))
                    };
                    items.push(create_snippet(
                        &f.name,
                        &format_func_signature(f),
                        &snippet,
                    ));
                }
            }

            // User-defined structs
            for s in &index.structs {
                if unique_names.insert(s.name.clone()) {
                    items.push(CompletionItem {
                        label: s.name.clone(),
                        kind: Some(CompletionItemKind::STRUCT),
                        detail: Some(format!("struct {}", s.name)),
                        ..Default::default()
                    });
                }
            }

            // User-defined interfaces
            for iface in &index.interfaces {
                if unique_names.insert(iface.name.clone()) {
                    items.push(CompletionItem {
                        label: iface.name.clone(),
                        kind: Some(CompletionItemKind::INTERFACE),
                        detail: Some(format!("interface {}", iface.name)),
                        ..Default::default()
                    });
                }
            }

            // User variables
            for v in &index.variables {
                if unique_names.insert(v.name.clone()) {
                    let detail = if v.is_param {
                        "Function parameter".to_string()
                    } else if let Some(ref ty) = v.inferred_type {
                        format!("Variable (: {})", ty)
                    } else {
                        "Variable".to_string()
                    };
                    items.push(CompletionItem {
                        label: v.name.clone(),
                        kind: Some(if v.is_param {
                            CompletionItemKind::FIELD
                        } else {
                            CompletionItemKind::VARIABLE
                        }),
                        detail: Some(detail),
                        ..Default::default()
                    });
                }
            }
        }

        Ok(Some(CompletionResponse::Array(items)))
    }
}

fn create_snippet(label: &str, detail: &str, snippet: &str) -> CompletionItem {
    CompletionItem {
        label: label.to_string(),
        kind: Some(CompletionItemKind::FUNCTION),
        detail: Some(detail.to_string()),
        insert_text: Some(snippet.to_string()),
        insert_text_format: Some(InsertTextFormat::SNIPPET),
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// Document Indexing
// ---------------------------------------------------------------------------

pub fn index_document(text: &str) -> DocumentIndex {
    let mut index = DocumentIndex::default();
    let lines: Vec<&str> = text.lines().collect();

    let mut lexer = Lexer::new(text);
    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        Err(_) => return index,
    };

    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i].kind {
            TokenKind::Function => {
                if i + 1 < tokens.len() {
                    if let TokenKind::Identifier(ref func_name) = tokens[i + 1].kind {
                        let line_0 = tokens[i + 1].line.saturating_sub(1) as u32;
                        let line_str = lines.get(line_0 as usize).unwrap_or(&"");
                        let col_0 = line_str.find(func_name.as_str()).unwrap_or(0) as u32;

                        let doc = extract_doc_comment(&lines, line_0 as usize);

                        let mut params = Vec::new();
                        let mut j = i + 2;
                        let mut in_parens = false;
                        let mut return_type = None;

                        while j < tokens.len() {
                            match &tokens[j].kind {
                                TokenKind::LParen => in_parens = true,
                                TokenKind::RParen => {
                                    if j + 1 < tokens.len() && tokens[j + 1].kind == TokenKind::Arrow {
                                        if j + 2 < tokens.len() {
                                            if let TokenKind::Identifier(ref ret_ty) = tokens[j + 2].kind {
                                                return_type = Some(ret_ty.clone());
                                            }
                                        }
                                    }
                                    break;
                                }
                                TokenKind::Identifier(p_name) if in_parens => {
                                    let mut p_type = None;
                                    if j + 2 < tokens.len() && tokens[j + 1].kind == TokenKind::Colon {
                                        if let TokenKind::Identifier(ty) = &tokens[j + 2].kind {
                                            p_type = Some(ty.clone());
                                            j += 2;
                                        }
                                    }
                                    params.push((p_name.clone(), p_type));

                                    index.variables.push(VarSymbol {
                                        name: p_name.clone(),
                                        line: tokens[j].line.saturating_sub(1) as u32,
                                        col: 0,
                                        is_param: true,
                                        inferred_type: None,
                                    });
                                }
                                TokenKind::LBrace => break,
                                _ => {}
                            }
                            j += 1;
                        }

                        index.functions.push(FnSymbol {
                            name: func_name.clone(),
                            params,
                            return_type,
                            line: line_0,
                            col: col_0,
                            doc,
                        });
                    }
                }
            }

            TokenKind::Struct => {
                if i + 1 < tokens.len() {
                    if let TokenKind::Identifier(ref struct_name) = tokens[i + 1].kind {
                        let line_0 = tokens[i + 1].line.saturating_sub(1) as u32;
                        let line_str = lines.get(line_0 as usize).unwrap_or(&"");
                        let col_0 = line_str.find(struct_name.as_str()).unwrap_or(0) as u32;
                        let doc = extract_doc_comment(&lines, line_0 as usize);

                        let mut fields = Vec::new();
                        let mut j = i + 2;
                        let mut in_brace = false;

                        while j < tokens.len() {
                            match &tokens[j].kind {
                                TokenKind::LBrace => in_brace = true,
                                TokenKind::RBrace => break,
                                TokenKind::Identifier(f_name) if in_brace => {
                                    if j + 2 < tokens.len() && tokens[j + 1].kind == TokenKind::Colon {
                                        if let TokenKind::Identifier(f_type) = &tokens[j + 2].kind {
                                            fields.push((f_name.clone(), f_type.clone()));
                                            j += 2;
                                        }
                                    }
                                }
                                _ => {}
                            }
                            j += 1;
                        }

                        index.structs.push(StructSymbol {
                            name: struct_name.clone(),
                            fields,
                            line: line_0,
                            col: col_0,
                            doc,
                        });
                        i = j;
                    }
                }
            }

            TokenKind::Impl => {
                if i + 1 < tokens.len() {
                    if let TokenKind::Identifier(ref struct_name) = tokens[i + 1].kind {
                        let line_0 = tokens[i + 1].line.saturating_sub(1) as u32;
                        let line_str = lines.get(line_0 as usize).unwrap_or(&"");
                        let col_0 = line_str.find(struct_name.as_str()).unwrap_or(0) as u32;

                        let mut methods = Vec::new();
                        let mut j = i + 2;
                        let mut brace_depth = 0;

                        while j < tokens.len() {
                            match &tokens[j].kind {
                                TokenKind::LBrace => brace_depth += 1,
                                TokenKind::RBrace => {
                                    brace_depth -= 1;
                                    if brace_depth == 0 {
                                        break;
                                    }
                                }
                                TokenKind::Function if brace_depth == 1 => {
                                    if j + 1 < tokens.len() {
                                        if let TokenKind::Identifier(ref m_name) = tokens[j + 1].kind {
                                            let m_line_0 = tokens[j + 1].line.saturating_sub(1) as u32;
                                            let m_line_str = lines.get(m_line_0 as usize).unwrap_or(&"");
                                            let m_col_0 = m_line_str.find(m_name.as_str()).unwrap_or(0) as u32;
                                            let m_doc = extract_doc_comment(&lines, m_line_0 as usize);

                                            let mut m_params = Vec::new();
                                            let mut k = j + 2;
                                            let mut in_m_parens = false;
                                            let mut m_return_type = None;

                                            while k < tokens.len() {
                                                match &tokens[k].kind {
                                                    TokenKind::LParen => in_m_parens = true,
                                                    TokenKind::RParen => {
                                                        if k + 1 < tokens.len() && tokens[k + 1].kind == TokenKind::Arrow {
                                                            if k + 2 < tokens.len() {
                                                                if let TokenKind::Identifier(ref ret_ty) = tokens[k + 2].kind {
                                                                    m_return_type = Some(ret_ty.clone());
                                                                }
                                                            }
                                                        }
                                                        break;
                                                    }
                                                    TokenKind::Identifier(p_name) if in_m_parens => {
                                                        let mut p_type = None;
                                                        if k + 2 < tokens.len() && tokens[k + 1].kind == TokenKind::Colon {
                                                            if let TokenKind::Identifier(ty) = &tokens[k + 2].kind {
                                                                p_type = Some(ty.clone());
                                                                k += 2;
                                                            }
                                                        }
                                                        m_params.push((p_name.clone(), p_type));
                                                    }
                                                    TokenKind::LBrace => break,
                                                    _ => {}
                                                }
                                                k += 1;
                                            }

                                            methods.push(FnSymbol {
                                                name: m_name.clone(),
                                                params: m_params,
                                                return_type: m_return_type,
                                                line: m_line_0,
                                                col: m_col_0,
                                                doc: m_doc,
                                            });
                                        }
                                    }
                                }
                                _ => {}
                            }
                            j += 1;
                        }

                        index.impls.push(ImplSymbol {
                            struct_name: struct_name.clone(),
                            methods,
                            line: line_0,
                            col: col_0,
                        });
                        i = j;
                    }
                }
            }

            TokenKind::Interface => {
                if i + 1 < tokens.len() {
                    if let TokenKind::Identifier(ref iface_name) = tokens[i + 1].kind {
                        let line_0 = tokens[i + 1].line.saturating_sub(1) as u32;
                        let line_str = lines.get(line_0 as usize).unwrap_or(&"");
                        let col_0 = line_str.find(iface_name.as_str()).unwrap_or(0) as u32;
                        let doc = extract_doc_comment(&lines, line_0 as usize);

                        let mut methods = Vec::new();
                        let mut j = i + 2;
                        let mut in_brace = false;

                        while j < tokens.len() {
                            match &tokens[j].kind {
                                TokenKind::LBrace => in_brace = true,
                                TokenKind::RBrace => break,
                                TokenKind::Identifier(m_name) if in_brace => {
                                    methods.push(m_name.clone());
                                }
                                _ => {}
                            }
                            j += 1;
                        }

                        index.interfaces.push(InterfaceSymbol {
                            name: iface_name.clone(),
                            methods,
                            line: line_0,
                            col: col_0,
                            doc,
                        });
                        i = j;
                    }
                }
            }

            TokenKind::Let => {
                if i + 1 < tokens.len() {
                    match &tokens[i + 1].kind {
                        TokenKind::Identifier(var_name) => {
                            let line_0 = tokens[i + 1].line.saturating_sub(1) as u32;
                            let line_str = lines.get(line_0 as usize).unwrap_or(&"");
                            let col_0 = line_str.find(var_name.as_str()).unwrap_or(0) as u32;

                            // Infer simple types for Inlay Hints
                            let mut inferred = None;
                            if i + 2 < tokens.len() && tokens[i + 2].kind == TokenKind::Assign {
                                if i + 3 < tokens.len() {
                                    match &tokens[i + 3].kind {
                                        TokenKind::Int(_) => inferred = Some("int".to_string()),
                                        TokenKind::Float(_) => inferred = Some("float".to_string()),
                                        TokenKind::Str(_) | TokenKind::FStr(_) => inferred = Some("str".to_string()),
                                        TokenKind::True | TokenKind::False => inferred = Some("bool".to_string()),
                                        TokenKind::Identifier(call) if call == "new" => {
                                            if i + 5 < tokens.len() {
                                                if let TokenKind::Identifier(ty) = &tokens[i + 5].kind {
                                                    inferred = Some(ty.clone());
                                                }
                                            }
                                        }
                                        TokenKind::Identifier(call) => {
                                            if let Some(f) = index.functions.iter().find(|f| &f.name == call) {
                                                if let Some(ref ret) = f.return_type {
                                                    inferred = Some(ret.clone());
                                                }
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }

                            index.variables.push(VarSymbol {
                                name: var_name.clone(),
                                line: line_0,
                                col: col_0,
                                is_param: false,
                                inferred_type: inferred,
                            });
                        }
                        TokenKind::LParen => {
                            let mut j = i + 2;
                            while j < tokens.len() && tokens[j].kind != TokenKind::RParen {
                                if let TokenKind::Identifier(v_name) = &tokens[j].kind {
                                    let line_0 = tokens[j].line.saturating_sub(1) as u32;
                                    let line_str = lines.get(line_0 as usize).unwrap_or(&"");
                                    let col_0 = line_str.find(v_name.as_str()).unwrap_or(0) as u32;
                                    index.variables.push(VarSymbol {
                                        name: v_name.clone(),
                                        line: line_0,
                                        col: col_0,
                                        is_param: false,
                                        inferred_type: None,
                                    });
                                }
                                j += 1;
                            }
                        }
                        _ => {}
                    }
                }
            }

            TokenKind::For => {
                if i + 1 < tokens.len() {
                    if let TokenKind::Identifier(ref iter_var) = tokens[i + 1].kind {
                        let line_0 = tokens[i + 1].line.saturating_sub(1) as u32;
                        let line_str = lines.get(line_0 as usize).unwrap_or(&"");
                        let col_0 = line_str.find(iter_var.as_str()).unwrap_or(0) as u32;

                        let mut inferred = None;
                        if i + 3 < tokens.len() {
                            if let TokenKind::Identifier(ref callee) = tokens[i + 3].kind {
                                if callee == "range" {
                                    inferred = Some("int".to_string());
                                }
                            } else if let TokenKind::Str(_) = tokens[i + 3].kind {
                                inferred = Some("str".to_string());
                            }
                        }

                        index.variables.push(VarSymbol {
                            name: iter_var.clone(),
                            line: line_0,
                            col: col_0,
                            is_param: false,
                            inferred_type: inferred,
                        });
                    }
                }
            }

            _ => {}
        }
        i += 1;
    }

    index
}

fn extract_doc_comment(lines: &[&str], target_line: usize) -> Option<String> {
    if target_line == 0 {
        return None;
    }

    let mut comment_lines = Vec::new();
    let mut curr = target_line;

    while curr > 0 {
        curr -= 1;
        let line = lines[curr].trim();
        if line.starts_with("//") {
            let text = line.trim_start_matches("//").trim();
            comment_lines.push(text.to_string());
        } else {
            break;
        }
    }

    if comment_lines.is_empty() {
        None
    } else {
        comment_lines.reverse();
        Some(comment_lines.join("\n"))
    }
}

// ---------------------------------------------------------------------------
// Word and Identifier Extraction
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct WordAtPos {
    word: String,
    range: Range,
    receiver: Option<String>,
}

fn get_word_at_position(text: &str, pos: Position) -> Option<WordAtPos> {
    let line_str = text.lines().nth(pos.line as usize)?;
    let chars: Vec<char> = line_str.chars().collect();
    let col = pos.character as usize;

    if chars.is_empty() {
        return None;
    }

    let col = col.min(chars.len().saturating_sub(1));

    let is_ident_char = |c: char| c.is_alphanumeric() || c == '_';

    if !is_ident_char(chars[col]) {
        if col > 0 && is_ident_char(chars[col - 1]) {
            return get_word_at_position(
                text,
                Position {
                    line: pos.line,
                    character: (col - 1) as u32,
                },
            );
        }
        return None;
    }

    let mut start = col;
    while start > 0 && is_ident_char(chars[start - 1]) {
        start -= 1;
    }

    let mut end = col;
    while end < chars.len() && is_ident_char(chars[end]) {
        end += 1;
    }

    let word: String = chars[start..end].iter().collect();

    // Check if preceded by dot: receiver.word
    let mut receiver = None;
    let mut idx = start;
    while idx > 0 && chars[idx - 1].is_whitespace() {
        idx -= 1;
    }

    if idx > 0 && chars[idx - 1] == '.' {
        let mut rec_end = idx - 1;
        while rec_end > 0 && chars[rec_end - 1].is_whitespace() {
            rec_end -= 1;
        }
        let mut rec_start = rec_end;
        while rec_start > 0 && is_ident_char(chars[rec_start - 1]) {
            rec_start -= 1;
        }
        if rec_start < rec_end {
            receiver = Some(chars[rec_start..rec_end].iter().collect());
        }
    }

    Some(WordAtPos {
        word,
        range: Range {
            start: Position {
                line: pos.line,
                character: start as u32,
            },
            end: Position {
                line: pos.line,
                character: end as u32,
            },
        },
        receiver,
    })
}

// ---------------------------------------------------------------------------
// Inlay Hints
// ---------------------------------------------------------------------------

pub fn compute_inlay_hints(_text: &str, index: &DocumentIndex) -> Vec<InlayHint> {
    let mut hints = Vec::new();

    for var in &index.variables {
        if !var.is_param {
            if let Some(ref ty) = var.inferred_type {
                hints.push(InlayHint {
                    position: Position {
                        line: var.line,
                        character: var.col + var.name.len() as u32,
                    },
                    label: InlayHintLabel::String(format!(": {}", ty)),
                    kind: Some(InlayHintKind::TYPE),
                    text_edits: None,
                    tooltip: Some(InlayHintTooltip::String(format!(
                        "Inferred type: {}",
                        ty
                    ))),
                    padding_left: Some(true),
                    padding_right: Some(false),
                    data: None,
                });
            }
        }
    }

    hints
}

// ---------------------------------------------------------------------------
// Hover Information (English)
// ---------------------------------------------------------------------------

pub fn get_hover_info(text: &str, pos: Position, index: &DocumentIndex) -> Option<Hover> {
    let word_info = get_word_at_position(text, pos)?;
    let word = word_info.word.as_str();
    let receiver = word_info.receiver.as_deref();

    let doc_str = if let Some(rec) = receiver {
        match (rec, word) {
            ("net", "listen") => {
                "```whalli\nfunc net.listen(port: int) -> int\n```\nBinds a non-blocking TCP server listener to `127.0.0.1:<port>` registered with the MIO asynchronous event loop.\n\n---\n\n### Examples\n```whalli\nimport net\n\nlet server = net.listen(8080)\nlet client = net.accept(server)\n```\n\n#### Parameters\n- `port`: The TCP port number to bind on localhost.\n\n#### Returns\n`int`: An integer descriptor token representing the server socket."
            }
            ("net", "accept") => {
                "```whalli\nfunc net.accept(server_id: int) -> int\n```\nAccepts an incoming TCP connection on the specified server socket. Suspends the current woroutine until a client connects.\n\n---\n\n### Examples\n```whalli\nimport net\n\nlet srv = net.listen(8080)\nlet client = net.accept(srv)\nprintln(\"New client connected:\", client)\n```\n\n#### Parameters\n- `server_id`: The server token ID returned from `net.listen`.\n\n#### Returns\n`int`: A client connection token ID for read/write operations."
            }
            ("net", "read") => {
                "```whalli\nfunc net.read(client_id: int) -> str\n```\nReads available data bytes (up to 4096 bytes) from the client socket. Suspends the current woroutine until data arrives.\n\n---\n\n### Examples\n```whalli\nimport net\n\nlet data = net.read(client)\nif data != nil {\n    println(\"Received:\", data)\n}\n```\n\n#### Parameters\n- `client_id`: The client socket token returned by `net.accept`.\n\n#### Returns\n`str`: Received data as a string, or `nil` if the client disconnected."
            }
            ("net", "write") => {
                "```whalli\nfunc net.write(client_id: int, data: str) -> bool\n```\nTransmits a string of bytes over the client TCP socket. Suspends the current woroutine if socket buffer is full.\n\n---\n\n### Examples\n```whalli\nimport net\n\nlet ok = net.write(client, \"HTTP/1.1 200 OK\\r\\n\\r\\nHello!\")\n```\n\n#### Parameters\n- `client_id`: The client socket token.\n- `data`: The string payload to send.\n\n#### Returns\n`bool`: `true` if transmission succeeded, `false` otherwise."
            }
            ("net", "close") => {
                "```whalli\nfunc net.close(client_id: int) -> bool\n```\nCloses the client connection and deregisters the stream from the event loop.\n\n---\n\n### Examples\n```whalli\nimport net\n\nnet.close(client)\n```\n\n#### Parameters\n- `client_id`: The socket token to close."
            }
            ("fs", "read") => {
                "```whalli\nfunc fs.read(path: str) -> str\n```\nReads the entire contents of a file at the specified path into a string.\n\n---\n\n### Examples\n```whalli\nimport fs\n\nlet content = fs.read(\"config.txt\")\nprintln(\"File content:\", content)\n```\n\n#### Parameters\n- `path`: File path relative to current working directory or absolute.\n\n#### Returns\n`str`: File contents as a string, or error string if read failed."
            }
            ("fs", "write") => {
                "```whalli\nfunc fs.write(path: str, data: str) -> bool\n```\nWrites data string to the specified file path, replacing existing contents or creating the file.\n\n---\n\n### Examples\n```whalli\nimport fs\n\nlet ok = fs.write(\"output.txt\", \"Hello World!\")\n```\n\n#### Parameters\n- `path`: Destination file path.\n- `data`: String content to write.\n\n#### Returns\n`bool`: `true` on success, `false` on I/O error."
            }
            ("time", "now") => {
                "```whalli\nfunc time.now() -> float\n```\nReturns the current UNIX timestamp in seconds with fractional floating-point precision.\n\n---\n\n### Examples\n```whalli\nimport time\n\nlet start = time.now()\n// do work...\nlet elapsed = time.now() - start\nprintln(\"Elapsed seconds:\", elapsed)\n```\n\n#### Returns\n`float`: Seconds elapsed since UNIX Epoch (January 1, 1970)."
            }
            ("time", "sleep") => {
                "```whalli\nfunc time.sleep(seconds: float)\n```\nNon-blocking sleep: suspends the current woroutine for `seconds` and yields execution to other tasks without blocking the thread pool.\n\n---\n\n### Examples\n```whalli\nimport time\n\ntime.sleep(1.5) // sleep for 1.5 seconds\nprintln(\"Awake!\")\n```\n\n#### Parameters\n- `seconds`: Duration to sleep in seconds (`float` or `int`)."
            }
            ("math", "pi") => {
                "```whalli\nconst math.pi: float = 3.141592653589793\n```\nThe mathematical constant $\\pi$, representing the ratio of the circumference of a circle to its diameter.\n\n---\n\n### Examples\n```whalli\nimport math\n\nlet radius = 5.0\nlet area = math.pi * radius * radius\n```"
            }
            ("math", "sin") => {
                "```whalli\nfunc math.sin(rad: float) -> float\n```\nComputes the trigonometric sine of an angle given in radians.\n\n---\n\n### Examples\n```whalli\nimport math\n\nlet y = math.sin(math.pi / 2.0) // 1.0\n```\n\n#### Parameters\n- `rad`: Angle in radians (`float`).\n\n#### Returns\n`float`: The sine of the angle in range `[-1.0, 1.0]`."
            }
            (_, "push") => {
                "```whalli\nmethod list.push(item: any)\n```\nAppends an element to the end of a `list`.\n\n---\n\n### Examples\n```whalli\nlet arr = new(\"list\")\narr.push(10)\narr.push(20)\nprintln(arr) // [10, 20]\n```\n\n#### Parameters\n- `item`: The element to insert."
            }
            (_, "pop") => {
                "```whalli\nmethod list.pop() -> any\n```\nRemoves and returns the last element from a `list`.\n\n---\n\n### Examples\n```whalli\nlet arr = new(\"list\")\narr.push(42)\nlet last = arr.pop() // 42\n```\n\n#### Returns\n`any`: The removed last element, or `nil` if the list was empty."
            }
            (_, "len") => {
                "```whalli\nmethod collection.len() -> int\n```\nReturns the number of elements in a collection (`list`, `map`) or character count of a `str`.\n\n---\n\n### Examples\n```whalli\nlet arr = new(\"list\")\narr.push(1)\nprintln(arr.len()) // 1\n\nlet s = \"hello\"\nprintln(s.len()) // 5\n```\n\n#### Returns\n`int`: The number of items in the collection."
            }
            (_, "keys") => {
                "```whalli\nmethod map.keys() -> list\n```\nReturns a new `list` containing all string keys present in the dictionary `map`.\n\n---\n\n### Examples\n```whalli\nlet m = new(\"map\")\nm[\"name\"] = \"Alice\"\nm[\"age\"] = 30\n\nfor k in m.keys() {\n    println(\"Key:\", k)\n}\n```\n\n#### Returns\n`list`: A list of all key strings."
            }
            (_, "remove") => {
                "```whalli\nmethod map.remove(key: str) -> any\n```\nRemoves the key-value mapping for `key` from the map and returns the deleted value.\n\n---\n\n### Examples\n```whalli\nlet m = new(\"map\")\nm[\"item\"] = 100\nlet deleted = m.remove(\"item\") // 100\n```\n\n#### Parameters\n- `key`: The key string to delete.\n\n#### Returns\n`any`: The value that was removed, or `nil` if not found."
            }
            _ => {
                for imp in &index.impls {
                    if imp.struct_name == rec {
                        for m in &imp.methods {
                            if m.name == word {
                                return Some(Hover {
                                    contents: HoverContents::Markup(MarkupContent {
                                        kind: MarkupKind::Markdown,
                                        value: format_func_signature(m),
                                    }),
                                    range: Some(word_info.range),
                                });
                            }
                        }
                    }
                }
                return None;
            }
        }
        .to_string()
    } else {
        match word {
            "net" => {
                "### Module `net`\nAsynchronous networking module for TCP communication built on top of `mio` non-blocking I/O.\n\n---\n\n### Examples\n```whalli\nimport net\n\nlet server = net.listen(8080)\nlet client = net.accept(server)\nlet request = net.read(client)\nnet.write(client, \"HTTP/1.1 200 OK\\r\\n\\r\\nHello!\")\nnet.close(client)\n```\n\n#### Members:\n- `listen(port: int) -> int`: Binds TCP server port\n- `accept(server_id: int) -> int`: Accepts client connection\n- `read(client_id: int) -> str`: Reads data from socket\n- `write(client_id: int, data: str) -> bool`: Writes data to socket\n- `close(client_id: int) -> bool`: Closes client connection".to_string()
            }
            "fs" => {
                "### Module `fs`\nFile system utilities for reading and writing files.\n\n---\n\n### Examples\n```whalli\nimport fs\n\nfs.write(\"data.txt\", \"Hello World!\")\nlet text = fs.read(\"data.txt\")\nprintln(text)\n```\n\n#### Members:\n- `read(path: str) -> str`: Reads file to string\n- `write(path: str, data: str) -> bool`: Writes string to file".to_string()
            }
            "time" => {
                "### Module `time`\nTime utilities and scheduler timer integration.\n\n---\n\n### Examples\n```whalli\nimport time\n\nlet start = time.now()\ntime.sleep(1.0)\nprintln(\"Elapsed:\", time.now() - start)\n```\n\n#### Members:\n- `now() -> float`: Returns current timestamp in seconds\n- `sleep(seconds: float)`: Suspends current woroutine".to_string()
            }
            "math" => {
                "### Module `math`\nMathematical functions and constants.\n\n---\n\n### Examples\n```whalli\nimport math\n\nlet y = math.sin(math.pi / 2.0)\nprintln(y) // 1.0\n```\n\n#### Members:\n- `pi: float`: Constant $\\pi$\n- `sin(rad: float) -> float`: Sine function".to_string()
            }
            "println" => {
                "```whalli\nfunc println(...args: any)\n```\nPrints string representations of arguments separated by space to standard output followed by a newline.\n\n---\n\n### Examples\n```whalli\nprintln(\"Hello, world!\")\nprintln(\"Count:\", 42, true)\n```".to_string()
            }
            "print" => {
                "```whalli\nfunc print(...args: any)\n```\nPrints string representations of arguments separated by space to standard output without a trailing newline.\n\n---\n\n### Examples\n```whalli\nprint(\"Loading: \")\nprintln(\"Done!\")\n```".to_string()
            }
            "range" => {
                "```whalli\nfunc range(start: int, end: int, step: int = 1) -> range\n```\nCreates a numeric range iterator from `start` up to (exclusive) `end` with step `step` for `for` loops.\n\n---\n\n### Examples\n```whalli\nfor i in range(0, 10, 2) {\n    println(i) // 0, 2, 4, 6, 8\n}\n\nfor i in range(5) {\n    println(i) // 0, 1, 2, 3, 4\n}\n```".to_string()
            }
            "new" => {
                "```whalli\nfunc new(type: str, length: int = 0, capacity: int = length) -> any\n```\nAllocates a new heap object: `\"list\"`, `\"map\"`, `\"chan\"` (communication channel for woroutines) or `\"tuple\"`.\n\n---\n\n### Examples\n```whalli\nlet list = new(\"list\")\nlet map = new(\"map\")\nlet ch = new(\"chan\", 10) // buffered channel with capacity 10\n```".to_string()
            }
            "len" => {
                "```whalli\nfunc len(collection: any) -> int\n```\nReturns the length of a `list`, `map`, or `str`.\n\n---\n\n### Examples\n```whalli\nlet items = new(\"list\")\nitems.push(1)\nprintln(len(items)) // 1\n```".to_string()
            }
            "int" => {
                "```whalli\nfunc int(val: any) -> int\n```\nCasts a number, boolean, or string to a 64-bit integer `int`.\n\n---\n\n### Examples\n```whalli\nlet n = int(\"123\") // 123\nlet f = int(3.99)  // 3\n```".to_string()
            }
            "float" => {
                "```whalli\nfunc float(val: any) -> float\n```\nCasts an integer or string to a 64-bit floating point number `float`.\n\n---\n\n### Examples\n```whalli\nlet x = float(42)    // 42.0\nlet y = float(\"3.14\") // 3.14\n```".to_string()
            }
            "str" => {
                "```whalli\nfunc str(val: any) -> str\n```\nConverts any value to its string representation.\n\n---\n\n### Examples\n```whalli\nlet s = str(123) // \"123\"\n```".to_string()
            }
            "bool" => {
                "```whalli\nfunc bool(val: any) -> bool\n```\nEvaluates the truthiness of a value, returning `true` or `false`.\n\n---\n\n### Examples\n```whalli\nlet b1 = bool(1)  // true\nlet b2 = bool(0)  // false\n```".to_string()
            }
            "wo" => {
                "### Keyword `wo` (woroutine)\nSpawns a function call into an asynchronous cooperative lightweight thread (woroutine).\nWoroutines yield cooperatively on channel I/O and non-blocking network operations without operating system thread overhead.\n\n---\n\n### Examples\n```whalli\nfunc worker(ch) {\n    ch <- \"work complete\"\n}\n\nlet ch = new(\"chan\", 1)\nwo worker(ch)\n\nlet result = <- ch\nprintln(result) // \"work complete\"\n```".to_string()
            }
            "func" => {
                "### Keyword `func`\nDefines a function with optional parameter typing and return type signature.\n\n---\n\n### Examples\n```whalli\nfunc calculate(a: int, b: int) -> int {\n    return a + b\n}\n\nlet sum = calculate(10, 20)\nprintln(sum) // 30\n```".to_string()
            }
            "let" => {
                "### Keyword `let`\nBinds a local or global variable, with optional tuple destructuring.\n\n---\n\n### Examples\n```whalli\nlet count = 42\nlet message = \"Hello Whalli\"\n\n// Tuple destructuring:\nlet (first, second) = get_pair()\n```".to_string()
            }
            "struct" => {
                "### Keyword `struct`\nDeclares a structured record data type with typed fields.\n\n---\n\n### Examples\n```whalli\nstruct Point {\n    x: int,\n    y: int,\n}\n\nstruct User {\n    id: int,\n    name: str,\n}\n```".to_string()
            }
            "impl" => {
                "### Keyword `impl`\nDefines member methods for a struct type.\n\n---\n\n### Examples\n```whalli\nstruct Point {\n    x: int,\n    y: int,\n}\n\nimpl Point {\n    func distance() -> float {\n        return 0.0\n    }\n}\n```".to_string()
            }
            "interface" => {
                "### Keyword `interface`\nDefines an abstract interface contract specifying required methods.\n\n---\n\n### Examples\n```whalli\ninterface Greeter {\n    func greet(),\n}\n```".to_string()
            }
            "is" => {
                "### Operator `is`\nChecks whether an object implements an interface or matches a runtime type.\n\n---\n\n### Examples\n```whalli\nif instance is Greeter {\n    instance.greet()\n}\n```".to_string()
            }
            "for" => {
                "### Keyword `for` (Loop)\nIterates over a collection, string characters, or numeric range.\n\n---\n\n### Examples\n```whalli\n// Range iteration:\nfor i in range(0, 10, 2) {\n    println(i)\n}\n\n// Collection iteration:\nfor item in my_list {\n    println(item)\n}\n\n// String iteration:\nfor char in \"Whalli\" {\n    println(char)\n}\n```".to_string()
            }
            "while" => {
                "### Keyword `while` (Loop)\nRepeats block execution as long as the condition evaluates to `true`.\n\n---\n\n### Examples\n```whalli\nlet x = 5\nwhile x > 0 {\n    println(x)\n    x -= 1\n}\n```".to_string()
            }
            "if" => {
                "### Keyword `if` (Condition)\nConditional branch execution based on a boolean expression.\n\n---\n\n### Examples\n```whalli\nif score >= 90 {\n    println(\"Grade: A\")\n} else {\n    println(\"Keep trying!\")\n}\n```".to_string()
            }
            "else" => "### Keyword `else`\nAlternative branch executed when preceding `if` condition evaluates to false.".to_string(),
            "return" => "### Keyword `return`\nExits the current function and passes back the specified result value.\n\n---\n\n### Examples\n```whalli\nfunc square(n: int) -> int {\n    return n * n\n}\n```".to_string(),
            "break" => "### Keyword `break`\nImmediately terminates the innermost `for` or `while` loop.".to_string(),
            "continue" => "### Keyword `continue`\nSkips the remainder of current iteration and proceeds to next loop cycle.".to_string(),
            "import" => "### Keyword `import`\nImports standard library modules (`net`, `fs`, `time`, `math`).\n\n---\n\n### Examples\n```whalli\nimport net\nimport time\n```".to_string(),
            "true" => "Boolean literal representing truth (`true`).".to_string(),
            "false" => "Boolean literal representing falsehood (`false`).".to_string(),
            "nil" => "Literal representing absence of value (`nil`).".to_string(),
            _ => {
                if let Some(f) = index.functions.iter().find(|f| f.name == word) {
                    format_func_signature(f)
                } else if let Some(s) = index.structs.iter().find(|s| s.name == word) {
                    format_struct_signature(s)
                } else if let Some(iface) = index.interfaces.iter().find(|i| i.name == word) {
                    format_interface_signature(iface)
                } else if let Some(v) = index.variables.iter().find(|v| v.name == word) {
                    let kind = if v.is_param {
                        "Function Parameter"
                    } else if let Some(ref ty) = v.inferred_type {
                        return Some(Hover {
                            contents: HoverContents::Markup(MarkupContent {
                                kind: MarkupKind::Markdown,
                                value: format!("```whalli\nlet {}: {}\n```\n---\nLocal Variable", v.name, ty),
                            }),
                            range: Some(word_info.range),
                        });
                    } else {
                        "Variable"
                    };
                    format!("```whalli\nlet {}\n```\n---\n{}", v.name, kind)
                } else {
                    return None;
                }
            }
        }
    };

    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: doc_str,
        }),
        range: Some(word_info.range),
    })
}

fn format_func_signature(f: &FnSymbol) -> String {
    let params_str: Vec<String> = f
        .params
        .iter()
        .map(|(name, ty)| match ty {
            Some(t) => format!("{}: {}", name, t),
            None => name.clone(),
        })
        .collect();

    let ret_str = match &f.return_type {
        Some(t) => format!(" -> {}", t),
        None => String::new(),
    };

    let mut doc = format!("```whalli\nfunc {}({}){}\n```", f.name, params_str.join(", "), ret_str);
    if let Some(ref d) = f.doc {
        doc.push_str("\n\n---\n");
        doc.push_str(d);
    }

    let p_names: Vec<String> = f.params.iter().map(|(p, _)| p.clone()).collect();
    doc.push_str("\n\n### Examples\n```whalli\n");
    if f.return_type.is_some() {
        doc.push_str(&format!("let result = {}({})\n", f.name, p_names.join(", ")));
    } else {
        doc.push_str(&format!("{}({})\n", f.name, p_names.join(", ")));
    }
    doc.push_str("```");
    doc
}

fn format_struct_signature(s: &StructSymbol) -> String {
    let mut fields_str = String::new();
    for (name, ty) in &s.fields {
        fields_str.push_str(&format!("    {}: {},\n", name, ty));
    }
    let mut doc = format!("```whalli\nstruct {} {{\n{}}}\n```", s.name, fields_str);
    if let Some(ref d) = s.doc {
        doc.push_str("\n\n---\n");
        doc.push_str(d);
    }

    doc.push_str(&format!("\n\n### Examples\n```whalli\nlet item = new(\"{}\")\n```", s.name));
    doc
}

fn format_interface_signature(iface: &InterfaceSymbol) -> String {
    let mut methods_str = String::new();
    for m in &iface.methods {
        methods_str.push_str(&format!("    {},\n", m));
    }
    let mut doc = format!("```whalli\ninterface {} {{\n{}}}\n```", iface.name, methods_str);
    if let Some(ref d) = iface.doc {
        doc.push_str("\n\n---\n");
        doc.push_str(d);
    }
    doc
}

// ---------------------------------------------------------------------------
// Definition Provider
// ---------------------------------------------------------------------------

pub fn find_definition(
    text: &str,
    pos: Position,
    uri: &Url,
    index: &DocumentIndex,
) -> Option<Location> {
    let word_info = get_word_at_position(text, pos)?;
    let word = word_info.word.as_str();

    if let Some(f) = index.functions.iter().find(|f| f.name == word) {
        return Some(Location {
            uri: uri.clone(),
            range: Range {
                start: Position {
                    line: f.line,
                    character: f.col,
                },
                end: Position {
                    line: f.line,
                    character: f.col + f.name.len() as u32,
                },
            },
        });
    }

    if let Some(s) = index.structs.iter().find(|s| s.name == word) {
        return Some(Location {
            uri: uri.clone(),
            range: Range {
                start: Position {
                    line: s.line,
                    character: s.col,
                },
                end: Position {
                    line: s.line,
                    character: s.col + s.name.len() as u32,
                },
            },
        });
    }

    if let Some(iface) = index.interfaces.iter().find(|i| i.name == word) {
        return Some(Location {
            uri: uri.clone(),
            range: Range {
                start: Position {
                    line: iface.line,
                    character: iface.col,
                },
                end: Position {
                    line: iface.line,
                    character: iface.col + iface.name.len() as u32,
                },
            },
        });
    }

    for imp in &index.impls {
        if let Some(m) = imp.methods.iter().find(|m| m.name == word) {
            return Some(Location {
                uri: uri.clone(),
                range: Range {
                    start: Position {
                        line: m.line,
                        character: m.col,
                    },
                    end: Position {
                        line: m.line,
                        character: m.col + m.name.len() as u32,
                    },
                },
            });
        }
    }

    if let Some(v) = index.variables.iter().find(|v| v.name == word) {
        return Some(Location {
            uri: uri.clone(),
            range: Range {
                start: Position {
                    line: v.line,
                    character: v.col,
                },
                end: Position {
                    line: v.line,
                    character: v.col + v.name.len() as u32,
                },
            },
        });
    }

    None
}

// ---------------------------------------------------------------------------
// Document Symbols
// ---------------------------------------------------------------------------

pub fn build_document_symbols(index: &DocumentIndex) -> Vec<DocumentSymbol> {
    let mut symbols = Vec::new();

    for f in &index.functions {
        let params_str: Vec<String> = f
            .params
            .iter()
            .map(|(p, ty)| match ty {
                Some(t) => format!("{}: {}", p, t),
                None => p.clone(),
            })
            .collect();
        let ret_str = f
            .return_type
            .as_ref()
            .map_or(String::new(), |t| format!(" -> {}", t));

        let detail = format!("({}){}", params_str.join(", "), ret_str);

        let range = Range {
            start: Position {
                line: f.line,
                character: f.col,
            },
            end: Position {
                line: f.line + 1,
                character: 0,
            },
        };

        #[allow(deprecated)]
        symbols.push(DocumentSymbol {
            name: f.name.clone(),
            detail: Some(detail),
            kind: SymbolKind::FUNCTION,
            tags: None,
            deprecated: None,
            range,
            selection_range: Range {
                start: Position {
                    line: f.line,
                    character: f.col,
                },
                end: Position {
                    line: f.line,
                    character: f.col + f.name.len() as u32,
                },
            },
            children: None,
        });
    }

    for s in &index.structs {
        let mut children = Vec::new();
        for (f_name, f_type) in &s.fields {
            #[allow(deprecated)]
            children.push(DocumentSymbol {
                name: f_name.clone(),
                detail: Some(f_type.clone()),
                kind: SymbolKind::FIELD,
                tags: None,
                deprecated: None,
                range: Range {
                    start: Position {
                        line: s.line,
                        character: s.col,
                    },
                    end: Position {
                        line: s.line,
                        character: s.col,
                    },
                },
                selection_range: Range {
                    start: Position {
                        line: s.line,
                        character: s.col,
                    },
                    end: Position {
                        line: s.line,
                        character: s.col,
                    },
                },
                children: None,
            });
        }

        let range = Range {
            start: Position {
                line: s.line,
                character: s.col,
            },
            end: Position {
                line: s.line + 1,
                character: 0,
            },
        };

        #[allow(deprecated)]
        symbols.push(DocumentSymbol {
            name: s.name.clone(),
            detail: Some("struct".to_string()),
            kind: SymbolKind::STRUCT,
            tags: None,
            deprecated: None,
            range,
            selection_range: Range {
                start: Position {
                    line: s.line,
                    character: s.col,
                },
                end: Position {
                    line: s.line,
                    character: s.col + s.name.len() as u32,
                },
            },
            children: if children.is_empty() {
                None
            } else {
                Some(children)
            },
        });
    }

    for imp in &index.impls {
        let mut children = Vec::new();
        for m in &imp.methods {
            let params_str: Vec<String> = m
                .params
                .iter()
                .map(|(p, ty)| match ty {
                    Some(t) => format!("{}: {}", p, t),
                    None => p.clone(),
                })
                .collect();
            let ret_str = m
                .return_type
                .as_ref()
                .map_or(String::new(), |t| format!(" -> {}", t));

            let detail = format!("({}){}", params_str.join(", "), ret_str);

            #[allow(deprecated)]
            children.push(DocumentSymbol {
                name: m.name.clone(),
                detail: Some(detail),
                kind: SymbolKind::METHOD,
                tags: None,
                deprecated: None,
                range: Range {
                    start: Position {
                        line: m.line,
                        character: m.col,
                    },
                    end: Position {
                        line: m.line + 1,
                        character: 0,
                    },
                },
                selection_range: Range {
                    start: Position {
                        line: m.line,
                        character: m.col,
                    },
                    end: Position {
                        line: m.line,
                        character: m.col + m.name.len() as u32,
                    },
                },
                children: None,
            });
        }

        let range = Range {
            start: Position {
                line: imp.line,
                character: imp.col,
            },
            end: Position {
                line: imp.line + 1,
                character: 0,
            },
        };

        #[allow(deprecated)]
        symbols.push(DocumentSymbol {
            name: format!("impl {}", imp.struct_name),
            detail: None,
            kind: SymbolKind::CLASS,
            tags: None,
            deprecated: None,
            range,
            selection_range: Range {
                start: Position {
                    line: imp.line,
                    character: imp.col,
                },
                end: Position {
                    line: imp.line,
                    character: imp.col + imp.struct_name.len() as u32,
                },
            },
            children: if children.is_empty() {
                None
            } else {
                Some(children)
            },
        });
    }

    for iface in &index.interfaces {
        let mut children = Vec::new();
        for m in &iface.methods {
            #[allow(deprecated)]
            children.push(DocumentSymbol {
                name: m.clone(),
                detail: None,
                kind: SymbolKind::METHOD,
                tags: None,
                deprecated: None,
                range: Range {
                    start: Position {
                        line: iface.line,
                        character: iface.col,
                    },
                    end: Position {
                        line: iface.line,
                        character: iface.col,
                    },
                },
                selection_range: Range {
                    start: Position {
                        line: iface.line,
                        character: iface.col,
                    },
                    end: Position {
                        line: iface.line,
                        character: iface.col,
                    },
                },
                children: None,
            });
        }

        let range = Range {
            start: Position {
                line: iface.line,
                character: iface.col,
            },
            end: Position {
                line: iface.line + 1,
                character: 0,
            },
        };

        #[allow(deprecated)]
        symbols.push(DocumentSymbol {
            name: iface.name.clone(),
            detail: Some("interface".to_string()),
            kind: SymbolKind::INTERFACE,
            tags: None,
            deprecated: None,
            range,
            selection_range: Range {
                start: Position {
                    line: iface.line,
                    character: iface.col,
                },
                end: Position {
                    line: iface.line,
                    character: iface.col + iface.name.len() as u32,
                },
            },
            children: if children.is_empty() {
                None
            } else {
                Some(children)
            },
        });
    }

    for v in &index.variables {
        if !v.is_param {
            let range = Range {
                start: Position {
                    line: v.line,
                    character: v.col,
                },
                end: Position {
                    line: v.line,
                    character: v.col + v.name.len() as u32,
                },
            };

            #[allow(deprecated)]
            symbols.push(DocumentSymbol {
                name: v.name.clone(),
                detail: v.inferred_type.clone(),
                kind: SymbolKind::VARIABLE,
                tags: None,
                deprecated: None,
                range,
                selection_range: range,
                children: None,
            });
        }
    }

    symbols
}

// ---------------------------------------------------------------------------
// Signature Help (English)
// ---------------------------------------------------------------------------

pub fn get_signature_help(text: &str, pos: Position, index: &DocumentIndex) -> Option<SignatureHelp> {
    let line_str = text.lines().nth(pos.line as usize)?;
    let col = pos.character as usize;

    let chars: Vec<char> = line_str.chars().take(col).collect();

    let mut paren_depth = 0;
    let mut comma_count = 0;
    let mut fn_call_end = None;

    let mut i = chars.len();
    while i > 0 {
        i -= 1;
        match chars[i] {
            ')' => paren_depth += 1,
            '(' => {
                if paren_depth == 0 {
                    fn_call_end = Some(i);
                    break;
                } else {
                    paren_depth -= 1;
                }
            }
            ',' if paren_depth == 0 => {
                comma_count += 1;
            }
            _ => {}
        }
    }

    let paren_pos = fn_call_end?;

    let mut fn_end = paren_pos;
    while fn_end > 0 && chars[fn_end - 1].is_whitespace() {
        fn_end -= 1;
    }

    let mut fn_start = fn_end;
    while fn_start > 0 && (chars[fn_start - 1].is_alphanumeric() || chars[fn_start - 1] == '_' || chars[fn_start - 1] == '.') {
        fn_start -= 1;
    }

    if fn_start == fn_end {
        return None;
    }

    let full_fn_name: String = chars[fn_start..fn_end].iter().collect();

    let sig_info = get_known_signature(&full_fn_name, index)?;

    Some(SignatureHelp {
        signatures: vec![sig_info],
        active_signature: Some(0),
        active_parameter: Some(comma_count as u32),
    })
}

fn get_known_signature(name: &str, index: &DocumentIndex) -> Option<SignatureInformation> {
    let (label, params, doc): (&str, Vec<&str>, &str) = match name {
        "println" => ("println(...args: any)", vec!["...args: any"], "Prints space-separated arguments to standard output with trailing newline"),
        "print" => ("print(...args: any)", vec!["...args: any"], "Prints space-separated arguments to standard output without trailing newline"),
        "range" => ("range(start: int, end: int, step: int = 1)", vec!["start: int", "end: int", "step: int = 1"], "Generates a numeric range iterator for `for` loops"),
        "new" => ("new(type: str, length: int = 0, capacity: int = length)", vec!["type: str", "length: int = 0", "capacity: int = length"], "Allocates a new heap object: list, map, chan, tuple"),
        "len" => ("len(collection: any) -> int", vec!["collection: any"], "Returns the length of a collection or string"),
        "int" => ("int(val: any) -> int", vec!["val: any"], "Converts value to 64-bit integer"),
        "float" => ("float(val: any) -> float", vec!["val: any"], "Converts value to 64-bit floating point number"),
        "str" => ("str(val: any) -> str", vec!["val: any"], "Converts value to string representation"),
        "bool" => ("bool(val: any) -> bool", vec!["val: any"], "Converts value to boolean true or false"),

        "net.listen" => ("net.listen(port: int) -> int", vec!["port: int"], "Starts non-blocking TCP server on 127.0.0.1"),
        "net.accept" => ("net.accept(server_id: int) -> int", vec!["server_id: int"], "Accepts incoming client connection"),
        "net.read" => ("net.read(client_id: int) -> str", vec!["client_id: int"], "Reads data string from client socket"),
        "net.write" => ("net.write(client_id: int, data: str) -> bool", vec!["client_id: int", "data: str"], "Writes data string to client socket"),
        "net.close" => ("net.close(client_id: int) -> bool", vec!["client_id: int"], "Closes client socket and removes from event loop"),

        "fs.read" => ("fs.read(path: str) -> str", vec!["path: str"], "Reads file contents into a string"),
        "fs.write" => ("fs.write(path: str, data: str) -> bool", vec!["path: str", "data: str"], "Writes data string to a file"),

        "time.now" => ("time.now() -> float", vec![], "Current UNIX timestamp in seconds"),
        "time.sleep" => ("time.sleep(seconds: float)", vec!["seconds: float"], "Suspends woroutine for specified duration"),

        "math.sin" => ("math.sin(rad: float) -> float", vec!["rad: float"], "Trigonometric sine function"),

        "push" => ("push(item: any)", vec!["item: any"], "Appends an element to a list"),
        "pop" => ("pop() -> any", vec![], "Removes and returns last element of a list"),
        "keys" => ("keys() -> list", vec![], "Returns all dictionary keys as a list"),
        "remove" => ("remove(key: str) -> any", vec!["key: str"], "Removes key and returns its value"),

        _ => {
            if let Some(f) = index.functions.iter().find(|f| f.name == name) {
                let params_info: Vec<ParameterInformation> = f
                    .params
                    .iter()
                    .map(|(p, ty)| {
                        let label = match ty {
                            Some(t) => format!("{}: {}", p, t),
                            None => p.clone(),
                        };
                        ParameterInformation {
                            label: ParameterLabel::Simple(label),
                            documentation: None,
                        }
                    })
                    .collect();

                let ret_str = f
                    .return_type
                    .as_ref()
                    .map_or(String::new(), |t| format!(" -> {}", t));

                let p_names: Vec<String> = f
                    .params
                    .iter()
                    .map(|(p, ty)| match ty {
                        Some(t) => format!("{}: {}", p, t),
                        None => p.clone(),
                    })
                    .collect();

                let label = format!("func {}({}){}", f.name, p_names.join(", "), ret_str);

                return Some(SignatureInformation {
                    label,
                    documentation: f.doc.as_ref().map(|d| {
                        Documentation::MarkupContent(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: d.clone(),
                        })
                    }),
                    parameters: Some(params_info),
                    active_parameter: None,
                });
            }
            return None;
        }
    };

    let params_info: Vec<ParameterInformation> = params
        .into_iter()
        .map(|p| ParameterInformation {
            label: ParameterLabel::Simple(p.to_string()),
            documentation: None,
        })
        .collect();

    Some(SignatureInformation {
        label: label.to_string(),
        documentation: Some(Documentation::MarkupContent(MarkupContent {
            kind: MarkupKind::Markdown,
            value: doc.to_string(),
        })),
        parameters: Some(params_info),
        active_parameter: None,
    })
}

// ---------------------------------------------------------------------------
// Semantic Tokens Provider
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct RawSemanticToken {
    line: u32,
    start_char: u32,
    length: u32,
    token_type: u32,
    token_modifiers: u32,
}

pub fn compute_semantic_tokens(text: &str) -> Vec<SemanticToken> {
    let mut raw_tokens = Vec::new();
    let lines: Vec<&str> = text.lines().collect();

    for (line_idx, line) in lines.iter().enumerate() {
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            let c = chars[i];

            if c.is_whitespace() {
                i += 1;
                continue;
            }

            // Comments
            if c == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
                raw_tokens.push(RawSemanticToken {
                    line: line_idx as u32,
                    start_char: i as u32,
                    length: (chars.len() - i) as u32,
                    token_type: 9, // COMMENT
                    token_modifiers: 0,
                });
                break;
            }

            // Strings
            if c == '"' || (c == 'f' && i + 1 < chars.len() && chars[i + 1] == '"') {
                let start = i;
                if c == 'f' {
                    i += 1;
                }
                i += 1;
                while i < chars.len() {
                    if chars[i] == '\\' && i + 1 < chars.len() {
                        i += 2;
                    } else if chars[i] == '"' {
                        i += 1;
                        break;
                    } else {
                        i += 1;
                    }
                }
                raw_tokens.push(RawSemanticToken {
                    line: line_idx as u32,
                    start_char: start as u32,
                    length: (i - start) as u32,
                    token_type: 6, // STRING
                    token_modifiers: 0,
                });
                continue;
            }

            // Numbers
            if c.is_ascii_digit() {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.' || chars[i] == '_') {
                    i += 1;
                }
                raw_tokens.push(RawSemanticToken {
                    line: line_idx as u32,
                    start_char: start as u32,
                    length: (i - start) as u32,
                    token_type: 7, // NUMBER
                    token_modifiers: 0,
                });
                continue;
            }

            // Operators / Arrows
            if c == '<' && i + 1 < chars.len() && chars[i + 1] == '-' {
                raw_tokens.push(RawSemanticToken {
                    line: line_idx as u32,
                    start_char: i as u32,
                    length: 2,
                    token_type: 8, // OPERATOR
                    token_modifiers: 0,
                });
                i += 2;
                continue;
            }

            if c == '-' && i + 1 < chars.len() && chars[i + 1] == '>' {
                raw_tokens.push(RawSemanticToken {
                    line: line_idx as u32,
                    start_char: i as u32,
                    length: 2,
                    token_type: 8, // OPERATOR
                    token_modifiers: 0,
                });
                i += 2;
                continue;
            }

            if ['+', '-', '*', '/', '%', '=', '!', '<', '>'].contains(&c) {
                let start = i;
                i += 1;
                if i < chars.len() && chars[i] == '=' {
                    i += 1;
                }
                raw_tokens.push(RawSemanticToken {
                    line: line_idx as u32,
                    start_char: start as u32,
                    length: (i - start) as u32,
                    token_type: 8, // OPERATOR
                    token_modifiers: 0,
                });
                continue;
            }

            // Identifiers / Keywords
            if c.is_alphanumeric() || c == '_' {
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();

                let mut prev_word = String::new();
                let mut k = start;
                while k > 0 && chars[k - 1].is_whitespace() {
                    k -= 1;
                }
                let mut p_end = k;
                while p_end > 0 && (chars[p_end - 1].is_alphanumeric() || chars[p_end - 1] == '_') {
                    p_end -= 1;
                }
                if p_end < k {
                    prev_word = chars[p_end..k].iter().collect();
                }

                let is_followed_by_paren = {
                    let mut next = i;
                    while next < chars.len() && chars[next].is_whitespace() {
                        next += 1;
                    }
                    next < chars.len() && chars[next] == '('
                };

                let (token_type, token_modifiers) = match word.as_str() {
                    "let" | "func" | "return" | "if" | "else" | "while" | "for" | "in"
                    | "break" | "continue" | "import" | "struct" | "impl" | "is"
                    | "interface" | "wo" | "true" | "false" | "nil" => (0, 0), // KEYWORD

                    "and" | "or" | "not" => (8, 0), // OPERATOR

                    "int" | "float" | "str" | "bool" | "list" | "map" | "chan" | "tuple" => (1, 0), // TYPE

                    "net" | "fs" | "time" | "math" => (3, 2), // VARIABLE + DEFAULT_LIBRARY

                    "println" | "print" | "range" | "new" | "len" => (2, 2), // FUNCTION + DEFAULT_LIBRARY

                    _ if prev_word == "func" => (2, 1), // FUNCTION + DECLARATION
                    _ if prev_word == "struct" => (10, 1), // STRUCT + DECLARATION
                    _ if prev_word == "interface" => (11, 1), // INTERFACE + DECLARATION
                    _ if prev_word == "let" || prev_word == "for" => (3, 1), // VARIABLE + DECLARATION
                    _ if is_followed_by_paren => (2, 0), // FUNCTION call
                    _ => (3, 0), // VARIABLE
                };

                raw_tokens.push(RawSemanticToken {
                    line: line_idx as u32,
                    start_char: start as u32,
                    length: (i - start) as u32,
                    token_type,
                    token_modifiers,
                });
                continue;
            }

            i += 1;
        }
    }

    raw_tokens.sort_by(|a, b| a.line.cmp(&b.line).then(a.start_char.cmp(&b.start_char)));

    let mut semantic_tokens = Vec::with_capacity(raw_tokens.len());
    let mut prev_line = 0;
    let mut prev_start = 0;

    for t in raw_tokens {
        let delta_line = t.line - prev_line;
        let delta_start = if delta_line == 0 {
            t.start_char - prev_start
        } else {
            t.start_char
        };

        semantic_tokens.push(SemanticToken {
            delta_line,
            delta_start,
            length: t.length,
            token_type: t.token_type,
            token_modifiers_bitset: t.token_modifiers,
        });

        prev_line = t.line;
        prev_start = t.start_char;
    }

    semantic_tokens
}

// ---------------------------------------------------------------------------
// Document Formatting
// ---------------------------------------------------------------------------

pub fn format_whalli_code(text: &str) -> String {
    let mut formatted_lines = Vec::new();
    let mut indent_level: usize = 0;
    let mut last_was_empty = false;

    for raw_line in text.lines() {
        let trimmed = raw_line.trim();

        if trimmed.is_empty() {
            if !last_was_empty && !formatted_lines.is_empty() {
                formatted_lines.push(String::new());
                last_was_empty = true;
            }
            continue;
        }
        last_was_empty = false;

        let close_at_start = trimmed.starts_with('}') || trimmed.starts_with(']');
        let line_indent = if close_at_start {
            indent_level.saturating_sub(1)
        } else {
            indent_level
        };

        let indent_str = "    ".repeat(line_indent);
        formatted_lines.push(format!("{}{}", indent_str, trimmed));

        let mut in_str = false;
        let mut escape = false;
        let mut open_count: usize = 0;
        let mut close_count: usize = 0;

        let chars: Vec<char> = trimmed.chars().collect();
        let mut idx = 0;

        while idx < chars.len() {
            let c = chars[idx];

            if in_str {
                if escape {
                    escape = false;
                } else if c == '\\' {
                    escape = true;
                } else if c == '"' {
                    in_str = false;
                }
            } else if c == '/' && idx + 1 < chars.len() && chars[idx + 1] == '/' {
                break;
            } else if c == '"' {
                in_str = true;
            } else if c == '{' {
                open_count += 1;
            } else if c == '}' {
                close_count += 1;
            }
            idx += 1;
        }

        indent_level = (indent_level + open_count).saturating_sub(close_count);
    }

    let mut result = formatted_lines.join("\n");
    if !result.is_empty() && !result.ends_with('\n') {
        result.push('\n');
    }
    result
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend {
        client,
        documents: Mutex::new(HashMap::new()),
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}

// ---------------------------------------------------------------------------
// Unit Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_indexing() {
        let code = r#"
// Greeter function
func greet(name: str, count: int) -> str {
    let msg = "Hello " + name
    return msg
}

struct Point {
    x: int,
    y: int,
}

impl Point {
    func distance() -> float {
        return 0.0
    }
}

interface Worker {
    work,
    stop
}
"#;
        let index = index_document(code);
        assert_eq!(index.functions.len(), 1);
        assert_eq!(index.functions[0].name, "greet");
        assert_eq!(index.functions[0].params.len(), 2);
        assert_eq!(index.functions[0].doc, Some("Greeter function".to_string()));

        assert_eq!(index.structs.len(), 1);
        assert_eq!(index.structs[0].name, "Point");
        assert_eq!(index.structs[0].fields.len(), 2);

        assert_eq!(index.impls.len(), 1);
        assert_eq!(index.impls[0].methods.len(), 1);
        assert_eq!(index.impls[0].methods[0].name, "distance");

        assert_eq!(index.interfaces.len(), 1);
        assert_eq!(index.interfaces[0].name, "Worker");
        assert_eq!(index.interfaces[0].methods.len(), 2);
    }

    #[test]
    fn test_hover_stdlib() {
        let code = "net.listen(8080)";
        let index = index_document(code);
        let hover = get_hover_info(
            code,
            Position {
                line: 0,
                character: 6,
            },
            &index,
        );
        assert!(hover.is_some());
        if let Some(h) = hover {
            if let HoverContents::Markup(m) = h.contents {
                assert!(m.value.contains("net.listen"));
            } else {
                panic!("Expected Markup hover");
            }
        }
    }

    #[test]
    fn test_goto_definition() {
        let code = "func my_func() {}\nmy_func()";
        let index = index_document(code);
        let uri = Url::parse("file:///test.wh").unwrap();
        let loc = find_definition(
            code,
            Position {
                line: 1,
                character: 2,
            },
            &uri,
            &index,
        );
        assert!(loc.is_some());
        let l = loc.unwrap();
        assert_eq!(l.range.start.line, 0);
    }

    #[test]
    fn test_signature_help() {
        let code = "range(0, 10, ";
        let index = index_document(code);
        let sig = get_signature_help(
            code,
            Position {
                line: 0,
                character: 13,
            },
            &index,
        );
        assert!(sig.is_some());
        let s = sig.unwrap();
        assert_eq!(s.active_parameter, Some(2));
    }

    #[test]
    fn test_semantic_tokens() {
        let code = "let x = 42\nfunc foo() {}";
        let tokens = compute_semantic_tokens(code);
        assert!(!tokens.is_empty());
    }

    #[test]
    fn test_formatting() {
        let unformatted = "for i in range(10){\nprintln(i)\n}";
        let formatted = format_whalli_code(unformatted);
        assert_eq!(formatted, "for i in range(10){\n    println(i)\n}\n");
    }

    #[test]
    fn test_document_symbols() {
        let code = r#"
func calculate(a: int, b: int) -> int {
    return a + b
}

struct User {
    id: int,
    name: str,
}
"#;
        let index = index_document(code);
        let symbols = build_document_symbols(&index);
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].name, "calculate");
        assert_eq!(symbols[1].name, "User");
    }

    #[test]
    fn test_hover_keyword_and_builtin() {
        let code = "wo println(42)";
        let index = index_document(code);
        let hover_wo = get_hover_info(code, Position { line: 0, character: 1 }, &index);
        assert!(hover_wo.is_some());
        if let Some(h) = hover_wo {
            if let HoverContents::Markup(m) = h.contents {
                assert!(m.value.contains("woroutine"));
            }
        }

        let hover_println = get_hover_info(code, Position { line: 0, character: 5 }, &index);
        assert!(hover_println.is_some());
        if let Some(h) = hover_println {
            if let HoverContents::Markup(m) = h.contents {
                assert!(m.value.contains("println"));
            }
        }
    }

    #[test]
    fn test_hover_user_function() {
        let code = r#"
// Performs calculation
func calculate(a: int, b: int) -> int {
    return a + b
}

calculate(1, 2)
"#;
        let index = index_document(code);
        let hover = get_hover_info(code, Position { line: 6, character: 3 }, &index);
        assert!(hover.is_some());
        if let Some(h) = hover {
            if let HoverContents::Markup(m) = h.contents {
                assert!(m.value.contains("func calculate(a: int, b: int) -> int"));
                assert!(m.value.contains("Performs calculation"));
            }
        }
    }

    #[test]
    fn test_inlay_hints() {
        let code = r#"
let num = 42
let text = "hello"
let flag = true
"#;
        let index = index_document(code);
        let hints = compute_inlay_hints(code, &index);
        assert_eq!(hints.len(), 3);
        assert!(match &hints[0].label {
            InlayHintLabel::String(s) => s == ": int",
            _ => false,
        });
        assert!(match &hints[1].label {
            InlayHintLabel::String(s) => s == ": str",
            _ => false,
        });
        assert!(match &hints[2].label {
            InlayHintLabel::String(s) => s == ": bool",
            _ => false,
        });
    }
}
