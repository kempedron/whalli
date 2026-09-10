use std::rc::Rc;

use crate::{ast::{BinaryOp, Expr, Stmt}, lexer::{TokenKind, Token}, value::Value};

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

    fn  current_line(&self) -> usize {
        self.tokens[self.pos].line
    }

    fn parse_statement(&mut self) -> Stmt {
        if self.match_token(TokenKind::Print){
            self.consume(TokenKind::LParen, "Expected '(' after print");
            
            let mut exprs = Vec::new();
            
            if !self.check_token(TokenKind::RParen) {
                loop {
                    exprs.push(self.parse_expression());
                    if !self.match_token(TokenKind::Comma){
                        break;
                    }
                }
            }
            
            self.consume(TokenKind::RParen, "expected '(' after print args");
            self.consume(TokenKind::Semicolon, "expected ';' after statement");
            return Stmt::Print(exprs);
        }

        if self.match_token(TokenKind::Let){
            let name_token = self.advance().clone();
            let name = match name_token {
                TokenKind::Identifier(n) => n,
                _ => panic!("[Line {}] Expect var name after 'let'",self.current_line())
            };
            self.consume(TokenKind::Assign, "Parser: Expected '=' sign after variable name");
            let expr = self.parse_expression();
            self.consume(TokenKind::Semicolon, "Parser: A semicolon ';' was expected after the expression");

            return Stmt::Let(name, expr);
        }
        panic!("[Line {}] Parser: expect statement, but got {:?}",self.current_line(), self.current_token())
    }

    fn parse_expression(&mut self) -> Expr {
        let mut left = self.parse_term();
        
        while self.match_token(TokenKind::Plus) || self.match_token(TokenKind::Sub){
            let op_token = self.previous_token().clone();
            let right = self.parse_term();
            let op = match op_token {
                TokenKind::Plus => BinaryOp::Add,
                TokenKind::Sub => BinaryOp::Sub,
                _ => unreachable!(),
            };
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        left
    }

    fn parse_term(&mut self) -> Expr{
        let mut left = self.parse_primary();

        while self.match_token(TokenKind::Star) || self.match_token(TokenKind::Slash){
            let op_token = self.previous_token().clone();
            let right = self.parse_primary();
            let op = match op_token {
                TokenKind::Star => BinaryOp::Mul,
                TokenKind::Slash => BinaryOp::Div,
                _ => unreachable!(),
            };
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        left
    }

    fn parse_primary(&mut self) -> Expr {
        if self.match_token(TokenKind::LParen) {
            let expr = self.parse_expression();
            self.consume(TokenKind::RParen,"Expect closing bracket ')'");
            return expr
        }
       
        let token = self.advance().clone();
        match token {
            TokenKind::Str(s) => Expr::Literal(Value::Str(Rc::new(s))),
            TokenKind::FStr(s) => Self::parse_interpolated_string(&s),
            TokenKind::Int(n) => Expr::Literal(Value::Int(n)),
            TokenKind::Float(n) => Expr::Literal(Value::Float(n)),
            TokenKind::Identifier(name) => Expr::Variable(name),
            _ => panic!("[Line {}] Parser: expect expression, got {:?}",self.current_line(), token),
        }
    }

    fn parse_interpolated_string(s: &str) -> Expr {
        let mut parts: Vec<Expr> = Vec::new();
        let chars: Vec<char> = s.chars().collect();
        let mut i =0;

        let mut current_literal = String::new();

        while i < chars.len() {
            if chars[i] == '{'  {
                if !current_literal.is_empty(){
                    parts.push(Expr::Literal(Value::Str(Rc::new(current_literal))));
                    current_literal = String::new();
                }
                i += 1;
                let mut var_name = String::new();
                while i <  chars.len() && chars[i] != '}' {
                    var_name.push(chars[i]);
                    i += 1;
                }
                if i < chars.len() { i += 1; }
                parts.push(Expr::Variable(var_name.trim().to_string()));
            } else {
                current_literal.push(chars[i]);
                i += 1
            }
        }
        if !current_literal.is_empty(){
            parts.push(Expr::Literal(Value::Str(Rc::new(current_literal))));
        }

        if parts.is_empty(){
            return Expr::Literal(Value::Str(Rc::new("".to_string())));
        }

        let mut expr = parts[0].clone();
        for next_part in parts.into_iter().skip(1) {
            expr = Expr::Binary(
                Box::new(expr),
                BinaryOp::Add,
                Box::new(next_part)
            );
        }
        expr


    }

    fn current_token(&self) -> &TokenKind {
        &self.tokens[self.pos].kind
    }

    fn previous_token(&self) -> &TokenKind {
        &self.tokens[self.pos - 1].kind
    }

    fn is_at_end(&self) -> bool {
        *self.current_token() == TokenKind::Eof
    }

    fn advance(&mut self) -> &TokenKind {
        if !self.is_at_end(){
            self.pos += 1;
        }
        &self.tokens[self.pos - 1].kind
    }

    fn match_token(&mut self, expected: TokenKind) -> bool {
        if *self.current_token() == expected {
            self.advance();
            true
        } else {
            false
        }
    }

    fn consume(&mut self, expected: TokenKind, err_msg: &str) {
        if self.match_token(expected) {
            return;
        } 
        panic!("Parser: {}",err_msg)
    }

    fn check_token(&self,expected: TokenKind) -> bool {
        if self.is_at_end(){
            return false;
        }
        *self.current_token() == expected
    }
}