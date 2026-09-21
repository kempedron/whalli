use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use whalli::lexer::{Lexer, TokenKind};

struct Backend {
    client: Client,
    documents: Mutex<HashMap<String, String>>,
}

impl Backend {
    fn get_line(&self, text: &str, line_idx: usize) -> String {
        text.lines().nth(line_idx).unwrap_or("").to_string()
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![".".to_string()]), 
                    ..Default::default()
                }),
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client.log_message(MessageType::INFO, "Whalli LSP started!").await;
    }

    async fn did_change(&self, mut params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.to_string();
        let new_text = params.content_changes.remove(0).text;
        let mut docs = self.documents.lock().unwrap();
        docs.insert(uri, new_text);
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri.to_string();
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

        let mut items = vec![];

        if is_dot_completion {            
            let caller_name = trimmed_before.trim_end_matches('.').split_whitespace().last().unwrap_or("");

            match caller_name {
                "net" => {
                    items.push(create_snippet("listen", "Слушать TCP порт", "listen(${1:port})"));
                    items.push(create_snippet("accept", "Принять подключение", "accept(${1:server_id})"));
                    items.push(create_snippet("read", "Читать из сокета", "read(${1:client_id})"));
                    items.push(create_snippet("write", "Писать в сокет", "write(${1:client_id}, ${2:data})"));
                    items.push(create_snippet("close", "Закрыть сокет", "close(${1:client_id})"));
                }
                "fs" => {
                    items.push(create_snippet("read", "Прочитать файл", "read(${1:\"path\"})"));
                    items.push(create_snippet("write", "Записать в файл", "write(${1:\"path\"}, ${2:data})"));
                }
                "time" => {
                    items.push(create_snippet("sleep", "Усыпить ворутину (сек)", "sleep(${1:seconds})"));
                    items.push(create_snippet("now", "Текущее время (мс)", "now()"));
                }
                "math" => {
                    items.push(CompletionItem { label: "pi".to_string(), kind: Some(CompletionItemKind::CONSTANT), ..Default::default() });
                    items.push(create_snippet("sin", "Синус", "sin(${1:rad})"));
                }
                _ => {
                    items.push(create_snippet("push", "Добавить элемент (List)", "push(${1:item})"));
                    items.push(create_snippet("pop", "Удалить последний элемент (List)", "pop()"));
                    items.push(create_snippet("len", "Длина коллекции", "len()"));
                    items.push(create_snippet("keys", "Ключи словаря (Map)", "keys()"));
                    items.push(create_snippet("remove", "Удалить по ключу (Map)", "remove(${1:key})"));
                }
            }
        } else {
            
            items.push(create_snippet("println", "Вывод в консоль", "println(${1:value})"));
            items.push(create_snippet("print", "Вывод без переноса", "print(${1:value})"));
            items.push(create_snippet("range", "Диапазон для for", "range(${1:start}, ${2:end}, ${3:step})"));

            for kw in ["int", "float", "str", "bool", "list", "map", "func", "chan", "true", "false", "nil", "wo"] {
                items.push(CompletionItem {
                    label: kw.to_string(),
                    kind: Some(CompletionItemKind::KEYWORD),
                    ..Default::default()
                });
            }

            for m in ["net", "fs", "time", "math"] {
                items.push(CompletionItem {
                    label: m.to_string(),
                    kind: Some(CompletionItemKind::MODULE),
                    ..Default::default()
                });
            }

            let mut lexer = Lexer::new(&text);
            let mut unique_names = HashSet::new();

            if let Ok(tokens) = lexer.tokenize() {
                let mut i = 0;
                while i < tokens.len() {
                    let current = &tokens[i].kind;

                    if *current == TokenKind::Let && i + 1 < tokens.len() {
                        if let TokenKind::Identifier(var_name) = &tokens[i + 1].kind {
                            if unique_names.insert(var_name.clone()) {
                                items.push(CompletionItem {
                                    label: var_name.clone(),
                                    kind: Some(CompletionItemKind::VARIABLE),
                                    detail: Some("Локальная переменная".to_string()),
                                    ..Default::default()
                                });
                            }
                        }
                    }
                    
                    if *current == TokenKind::Function && i + 1 < tokens.len() {
                        if let TokenKind::Identifier(func_name) = &tokens[i + 1].kind {
                            if unique_names.insert(func_name.clone()) {
                                items.push(create_snippet(
                                    func_name, 
                                    "Функция", 
                                    &format!("{}(${{1}})", func_name)
                                ));
                            }
                        }
                        
                        let mut j = i + 2;
                        let mut in_params = false;
                        while j < tokens.len() {
                            match &tokens[j].kind {
                                TokenKind::LParen => in_params = true,
                                TokenKind::RParen => break,
                                TokenKind::Identifier(arg_name) if in_params => {
                                    if unique_names.insert(arg_name.clone()) {
                                        items.push(CompletionItem {
                                            label: arg_name.clone(),
                                            kind: Some(CompletionItemKind::FIELD),
                                            detail: Some("Аргумент функции".to_string()),
                                            ..Default::default()
                                        });
                                    }
                                }
                                TokenKind::LBrace => break, 
                                _ => {}
                            }
                            j += 1;
                        }
                    }

                    if *current == TokenKind::For && i + 1 < tokens.len() {
                        if let TokenKind::Identifier(iter_var) = &tokens[i + 1].kind {
                             if unique_names.insert(iter_var.clone()) {
                                items.push(CompletionItem {
                                    label: iter_var.clone(),
                                    kind: Some(CompletionItemKind::VARIABLE),
                                    detail: Some("Переменная цикла".to_string()),
                                    ..Default::default()
                                });
                            }
                        }
                    }

                    i += 1;
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