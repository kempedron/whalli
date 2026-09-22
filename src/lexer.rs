#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Let,
    Assign, // =
    Plus,
    Sub,
    Mul,
    Div,
    Mod,
    PlusAssign,
    SubAssign,
    MulAssign,
    DivAssign,
    ModAssign,
    LParen,    // (
    RParen,    // )
    Semicolon, // ;
    NewLine,   // \n
    Comma,     // ,
    Colon,     // :
    Dot,       // .
    Float(f64),
    Int(i64),
    True,
    False,
    Str(String),
    FStr(String),
    Identifier(String),
    Function,
    LBrace, // {
    RBrace, // }
    Return,
    If,
    Else,
    And,
    Or,
    Not,
    BangEqual, // !=
    Equal,
    Less,         // <
    Greater,      // >
    LessEqual,    // <=
    GreaterEqual, // >=
    While,
    LBracket, // [
    RBracket, // ]
    For,
    In,
    Break,
    Continue,
    Import,
    Struct,
    Impl,
    Is,
    Interface,
    Arrow,  // -> (to specify the value to be returned)
    Wo,     // woroutines (lightweight threads)
    LArrow, // <-
    Nil,
    Error(String),
    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
}

#[derive(Debug)]
pub struct LexError {
    pub message: String,
    pub line: usize,
}

pub struct Lexer {
    chars: Vec<char>,
    pos: usize,
    line: usize,
    nesting: usize,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Lexer {
            chars: input.chars().collect(),
            pos: 0,
            line: 1,
            nesting: 0,
        }
    }

    fn make_token(&self, kind: TokenKind) -> Token {
        Token {
            kind,
            line: self.line,
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();
        while self.pos < self.chars.len() {
            let current = self.chars[self.pos];

            match current {
                ' ' | '\t' | '\r' => self.pos += 1,

                '\n' => {
                    self.pos += 1;
                    self.line += 1;
                    if self.nesting == 0 {
                        tokens.push(self.make_token(TokenKind::NewLine));
                    }
                }
                '(' => {
                    tokens.push(self.make_token(TokenKind::LParen));
                    self.nesting += 1;
                    self.pos += 1;
                }
                ')' => {
                    tokens.push(self.make_token(TokenKind::RParen));
                    if self.nesting > 0 {
                        self.nesting -= 1;
                    }
                    self.pos += 1;
                }
                '{' => {
                    tokens.push(self.make_token(TokenKind::LBrace));
                    self.pos += 1;
                }
                '}' => {
                    tokens.push(self.make_token(TokenKind::RBrace));
                    self.pos += 1;
                }
                '[' => {
                    tokens.push(self.make_token(TokenKind::LBracket));
                    self.nesting += 1;
                    self.pos += 1;
                }
                ']' => {
                    tokens.push(self.make_token(TokenKind::RBracket));
                    if self.nesting > 0 {
                        self.nesting -= 1;
                    }
                    self.pos += 1;
                }
                ':' => {
                    tokens.push(self.make_token(TokenKind::Colon));
                    self.pos += 1;
                }
                '.' => {
                    tokens.push(self.make_token(TokenKind::Dot));
                    self.pos += 1;
                }
                '+' => {
                    if self.pos + 1 < self.chars.len() && self.chars[self.pos + 1] == '=' {
                        self.pos += 2;
                        tokens.push(self.make_token(TokenKind::PlusAssign));
                    } else {
                        tokens.push(self.make_token(TokenKind::Plus));
                        self.pos += 1;
                    }
                }
                '*' => {
                    if self.pos + 1 < self.chars.len() && self.chars[self.pos + 1] == '=' {
                        self.pos += 2;
                        tokens.push(self.make_token(TokenKind::MulAssign));
                    } else {
                        tokens.push(self.make_token(TokenKind::Mul));
                        self.pos += 1;
                    }
                }
                '-' => {
                    if self.pos + 1 < self.chars.len() && self.chars[self.pos + 1] == '>' {
                        self.pos += 2;
                        tokens.push(self.make_token(TokenKind::Arrow));
                    } else if self.pos + 1 < self.chars.len() && self.chars[self.pos + 1] == '=' {
                        self.pos += 2;
                        tokens.push(self.make_token(TokenKind::SubAssign));
                    } else {
                        tokens.push(self.make_token(TokenKind::Sub));
                        self.pos += 1;
                    }
                }
                '/' => {
                    if self.pos + 1 < self.chars.len() && self.chars[self.pos + 1] == '/' {
                        while self.pos < self.chars.len() && self.chars[self.pos] != '\n' {
                            self.pos += 1;
                        }
                    } else if self.pos + 1 < self.chars.len() && self.chars[self.pos + 1] == '=' {
                        self.pos += 2;
                        tokens.push(self.make_token(TokenKind::DivAssign));
                    } else {
                        tokens.push(self.make_token(TokenKind::Div));
                        self.pos += 1;
                    }
                }
                '%' => {
                    if self.pos + 1 < self.chars.len() && self.chars[self.pos + 1] == '=' {
                        self.pos += 2;
                        tokens.push(self.make_token(TokenKind::ModAssign));
                    } else {
                        tokens.push(self.make_token(TokenKind::Mod));
                        self.pos += 1;
                    }
                }
                '=' => {
                    if self.pos + 1 < self.chars.len() && self.chars[self.pos + 1] == '=' {
                        self.pos += 2;
                        tokens.push(self.make_token(TokenKind::Equal));
                    } else {
                        self.pos += 1;
                        tokens.push(self.make_token(TokenKind::Assign));
                    }
                }
                '!' => {
                    if self.pos + 1 < self.chars.len() && self.chars[self.pos + 1] == '=' {
                        self.pos += 2;
                        tokens.push(self.make_token(TokenKind::BangEqual));
                    } else {
                        self.pos += 1;
                        tokens.push(self.make_token(TokenKind::Error("Unexpected char '!'".to_string())));
                    }
                }
                '>' => {
                    if self.pos + 1 < self.chars.len() && self.chars[self.pos + 1] == '=' {
                        tokens.push(self.make_token(TokenKind::GreaterEqual));
                        self.pos += 2;
                    } else {
                        tokens.push(self.make_token(TokenKind::Greater));
                        self.pos += 1;
                    }
                }
                '<' => {
                    if self.pos + 1 < self.chars.len() && self.chars[self.pos + 1] == '-' {
                        tokens.push(self.make_token(TokenKind::LArrow));
                        self.pos += 2;
                    } else if self.pos + 1 < self.chars.len() && self.chars[self.pos + 1] == '=' {
                        tokens.push(self.make_token(TokenKind::LessEqual));
                        self.pos += 2;
                    } else {
                        tokens.push(self.make_token(TokenKind::Less));
                        self.pos += 1
                    }
                }
                ';' => {
                    tokens.push(self.make_token(TokenKind::Semicolon));
                    self.pos += 1;
                }
                ',' => {
                    tokens.push(self.make_token(TokenKind::Comma));
                    self.pos += 1;
                }
                'f' => {
                    if self.pos + 1 < self.chars.len() && self.chars[self.pos + 1] == '"' {
                        self.pos += 2;
                        let mut s = String::new();
                        while self.pos < self.chars.len() && self.chars[self.pos] != '"' {
                            let ch = self.chars[self.pos];
                            if ch == '\\' {
                                self.pos += 1;
                                if self.pos < self.chars.len() {
                                    match self.chars[self.pos] {
                                        'n' => s.push('\n'),
                                        't' => s.push('\t'),
                                        '"' => s.push('"'),
                                        other => {
                                            s.push('\\');
                                            s.push(other);
                                        }
                                    }
                                }
                            } else {
                                s.push(ch);
                            }
                            self.pos += 1;
                        }
                        self.pos += 1;
                        tokens.push(self.make_token(TokenKind::FStr(s)));
                    } else {
                        let word = self.read_word();
                        let kind = Self::ident_or_keyword(word);
                        tokens.push(self.make_token(kind));
                    }
                }
                '"' => {
                    self.pos += 1;
                    let mut s = String::new();
                    while self.pos < self.chars.len() && self.chars[self.pos] != '"' {
                        let ch = self.chars[self.pos];
                        if ch == '\\' {
                            self.pos += 1;
                            if self.pos < self.chars.len() {
                                match self.chars[self.pos] {
                                    'n' => s.push('\n'),
                                    't' => s.push('\t'),
                                    '"' => s.push('"'),
                                    other => {
                                        s.push('\\');
                                        s.push(other);
                                    }
                                }
                            }
                        } else {
                            s.push(ch);
                        }
                        self.pos += 1;
                    }
                    self.pos += 1;
                    tokens.push(self.make_token(TokenKind::Str(s)));
                }

                c if c.is_ascii_digit() => {
                    tokens.push(self.read_number());
                }
                c if c.is_ascii_alphabetic() || c == '_' => {
                    let word = self.read_word();
                    let kind = Self::ident_or_keyword(word);
                    tokens.push(self.make_token(kind));
                }
                other => {
                    self.pos += 1;
                    tokens.push(self.make_token(TokenKind::Error(format!("Unknown char: {}", other))));
                }
            }
        }
        tokens.push(self.make_token(TokenKind::Eof));
        Ok(tokens)
    }

    fn ident_or_keyword(word: String) -> TokenKind {
        match word.as_str() {
            "let" => TokenKind::Let,
            "func" => TokenKind::Function,
            "return" => TokenKind::Return,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "while" => TokenKind::While,
            "for" => TokenKind::For,
            "in" => TokenKind::In,
            "and" => TokenKind::And,
            "or" => TokenKind::Or,
            "not" => TokenKind::Not,
            "break" => TokenKind::Break,
            "continue" => TokenKind::Continue,
            "import" => TokenKind::Import,
            "struct" => TokenKind::Struct,
            "impl" => TokenKind::Impl,
            "is" => TokenKind::Is,
            "nil" => TokenKind::Nil,
            "interface" => TokenKind::Interface,
            "wo" => TokenKind::Wo,
            _ => TokenKind::Identifier(word),
        }
    }

    fn read_number(&mut self) -> Token {
        let mut num_str = String::new();
        let mut is_float = false;

        while self.pos < self.chars.len()
            && (self.chars[self.pos].is_ascii_digit()
                || self.chars[self.pos] == '.'
                || self.chars[self.pos] == '_')
        {
            let ch = self.chars[self.pos];
            if ch == '_' {
                self.pos += 1;
                continue;
            }
            if ch == '.' {
                is_float = true;
            }
            num_str.push(ch);
            self.pos += 1;
        }
        if is_float {
            let val = num_str.parse::<f64>().expect("Error parsing Float");
            self.make_token(TokenKind::Float(val))
        } else {
            let val = num_str.parse::<i64>().expect("Error parse int");
            self.make_token(TokenKind::Int(val))
        }
    }

    fn read_word(&mut self) -> String {
        let mut word = String::new();
        while self.pos < self.chars.len()
            && (self.chars[self.pos].is_alphanumeric() || self.chars[self.pos] == '_')
        {
            word.push(self.chars[self.pos]);
            self.pos += 1;
        }
        word
    }
}
