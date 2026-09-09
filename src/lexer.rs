#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Print,
    Plus,
    Semicolon,
    Number(f64),
    Eof,
}

pub struct Lexer {
    chars: Vec<char>,
    pos: usize,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Lexer { chars: input.chars().collect(), pos: 0 }
    }

    pub fn tokenize(&mut self) -> Vec<Token>{
        let mut tokens = Vec::new();
        while self.pos < self.chars.len() {
            let current = self.chars[self.pos];

            match current {
                ' ' | '\t' | '\n' | '\r' => self.pos += 1,

                '+' => {
                    tokens.push(Token::Plus);
                    self.pos += 1;
                }
                ';'=> {
                    tokens.push(Token::Semicolon);
                    self.pos += 1;
                }
                c if c.is_ascii_digit() => {
                    tokens.push(Token::Number(self.read_number()));
                }
                c if c.is_ascii_alphabetic() => {
                    let word = self.read_word();
                    match word.as_str() {
                        "print" => tokens.push(Token::Print),
                        _ => panic!("lexer: undefined word {}", word),
                    }
                }
                _ => panic!("lexer: undefined char {}", current),
            }
        }
        tokens.push(Token::Eof);
        tokens
    }

    fn read_number(&mut self) -> f64 {
        let mut num_str = String::new();
        while self.pos < self.chars.len() && (self.chars[self.pos].is_ascii_digit() || self.chars[self.pos] == '.'){
            num_str.push(self.chars[self.pos]);
            self.pos += 1;
        }
        num_str.parse::<f64>().expect("Error digit parse")
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

