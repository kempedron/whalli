#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Print,
    Let,
    Assign, // =
    Plus,
    Sub,
    Star,
    Slash,
    LParen, // (
    RParen, // )
    Semicolon, // ;
    Comma, // ,
    Float(f64),
    Int(i64),
    True,
    False,
    Str(String),
    FStr(String),
    // List,
    Identifier(String), // var name
    Function,
    LBrace, // {
    RBrace, // }
    Return,
    If,
    Else,
    Equal,
    Less, // <
    Greater, // >
    While,
    LBracket, // [
    RBracket, // ]
    For,
    In,
    Eof,
}
#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
}

pub struct Lexer {
    chars: Vec<char>,
    pos: usize,
    line: usize,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Lexer { chars: input.chars().collect(), pos: 0, line: 1 }
    }

    fn make_token(&self, kind: TokenKind) -> Token {
        Token { kind, line: self.line }
    }

    pub fn tokenize(&mut self) -> Vec<Token>{
        let mut tokens = Vec::new();
        while self.pos < self.chars.len() {
            let current = self.chars[self.pos];

            match current {
                ' ' | '\t' | '\r' => self.pos += 1,

                '\n' => {
                    self.pos += 1;
                    self.line += 1;
                },

                '+' => {
                    tokens.push(self.make_token(TokenKind::Plus));
                    self.pos += 1;
                }
                '*' => {
                    tokens.push(self.make_token(TokenKind::Star));
                    self.pos += 1;
                }
                '-' => {
                    tokens.push(self.make_token(TokenKind::Sub));
                    self.pos += 1;
                }
                '/' => {
                    tokens.push(self.make_token(TokenKind::Slash));
                    self.pos += 1;
                }
                '=' => {
                    if self.pos + 1 < self.chars.len() && self.chars[self.pos + 1] == '='{
                        self.pos += 2;
                        tokens.push(self.make_token(TokenKind::Equal));
                    } else {
                        self.pos += 1;
                        tokens.push(self.make_token(TokenKind::Assign));
                    }
                }
                '>' => {
                    tokens.push(self.make_token(TokenKind::Greater));
                    self.pos += 1;
                }
                '<' => {
                    tokens.push(self.make_token(TokenKind::Less));
                    self.pos += 1;
                }
                ';'=> {
                    tokens.push(self.make_token(TokenKind::Semicolon));
                    self.pos += 1;
                }
                ',' => {
                    tokens.push(self.make_token(TokenKind::Comma));
                    self.pos += 1;
                }
                '(' => {
                    tokens.push(self.make_token(TokenKind::LParen));
                    self.pos += 1;
                }
                ')' => {
                    tokens.push(self.make_token(TokenKind::RParen));
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
                    self.pos += 1;
                }
                ']' => {
                    tokens.push(self.make_token(TokenKind::RBracket));
                    self.pos += 1;
                }
                'f' => {
                    if self.pos + 1 < self.chars.len() && self.chars[self.pos + 1] == '"' {
                        self.pos += 2;
                        let mut s = String::new();
                        while self.pos < self.chars.len() && self.chars[self.pos] != '"' {
                            let ch = self.chars[self.pos];
                            if ch  == '\\' {
                            self.pos += 1;
                            if self.pos < self.chars.len(){
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
                        if ch  == '\\' {
                            self.pos += 1;
                            if self.pos < self.chars.len(){
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
                c if c.is_ascii_alphabetic() => {
                    let word = self.read_word();
                    let kind = Self::ident_or_keyword(word);
                    tokens.push(self.make_token(kind));
                }
                _ => panic!("lexer: undefined char {}", current),
            }
        }
        tokens.push(self.make_token(TokenKind::Eof));
        tokens
    }

    fn ident_or_keyword(word: String) -> TokenKind {
        match word.as_str() {
            "print" => TokenKind::Print,
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
            _ => TokenKind::Identifier(word),
        }
    }

    fn read_number(&mut self) -> Token {
        let mut num_str = String::new();
        let mut is_float = false;
        
        while self.pos < self.chars.len() && (self.chars[self.pos].is_ascii_digit() || self.chars[self.pos] == '.'){
            let ch = self.chars[self.pos];
            if ch == '.' {
                is_float = true;
            }
            num_str.push(ch);
            self.pos+=1;
        }
        if is_float{
            let val = num_str.parse::<f64>().expect("Error parsing Float");
            self.make_token(TokenKind::Float(val))
        } else {
            let val = num_str.parse::<i64>().expect("Error parse int");
            self.make_token(TokenKind::Int(val))
        }

    }
    
    fn read_word(&mut self) -> String {
        let mut word = String::new();
        while self.pos < self.chars.len() && self.chars[self.pos].is_alphabetic() {
            word.push(self.chars[self.pos]);
            self.pos += 1
        }
        word
    }
}

