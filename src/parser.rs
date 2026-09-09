use crate::{ast::{Expr, Stmt,BinaryOp}, value::Value,lexer::Token};

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    pub fn parse(&mut self) -> Vec<Stmt>{
        let mut statements = Vec::new();
        while !self.is_at_end(){
            statements.push(self.parse_statement());
        }
        statements
    }

    fn parse_statement(&mut self) -> Stmt {
        if self.match_token(Token::Print){
            let expr = self.parse_expression();
            self.consume(Token::Semicolon, "expected ';' after statement");
            return Stmt::Print((expr))
        }
        panic!("Parser: expect statement, but got {:?}",self.current_token())
    }

    fn parse_expression(&mut self) -> Expr {
        let mut left = self.parse_primary();
        
        while self.match_token(Token::Plus){
            let right = self.parse_primary();
            left = Expr::Binary(Box::new(left), BinaryOp::Add, Box::new(right));
        }
        left
    }

    fn parse_primary(&mut self) -> Expr {
        let token = self.advance().clone();
        match token {
            Token::Number(n) => Expr::Literal((Value::Num((n)))),
            _ => panic!("Parser: expect expression, got {:?}",token),
        }
    }

    fn current_token(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn is_at_end(&self) -> bool {
        *self.current_token() == Token::Eof
    }

    fn advance(&mut self) -> &Token {
        if !self.is_at_end(){
            self.pos += 1;
        }
        &self.tokens[self.pos - 1]
    }

    fn match_token(&mut self, expected: Token) -> bool {
        if *self.current_token() == expected {
            self.advance();
            true
        } else {
            false
        }
    }

    fn consume(&mut self, expected: Token, err_msg: &str) {
        if self.match_token(expected) {
            return;
        } 
        panic!("Parser: {}",err_msg)
    }
}