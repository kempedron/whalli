use std::rc::Rc;

use crate::{ast::{BinaryOp, Expr, Stmt}, lexer::{Token, TokenKind}, value::Value};

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

        if self.match_token(TokenKind::Function) {
            let name_token = self.advance().clone();
            let name = match name_token{
                TokenKind::Identifier(n) => n,
                _ => panic!("[Line {}] Expect function name after 'func'", self.current_line()),
            };

            self.consume(TokenKind::LParen,"Expected '(' after function name");
            let mut params: Vec<String> = Vec::new();
            if !self.check_token(TokenKind::RParen) {
                loop {
                    let param_token = self.advance().clone();
                    match param_token {
                        TokenKind::Identifier(n) => params.push(n),
                        _ => panic!("[Line {}] Expect parameter name", self.current_line()),
                    }
                    if !self.match_token(TokenKind::Comma){
                        break;
                    }
                }
            }
            self.consume(TokenKind::RParen, "Expected ')' after parameters");

            self.consume(TokenKind::LBrace, "Expected '{' before function body");
            let mut body = Vec::new();
            while !self.check_token(TokenKind::RBrace) && !self.is_at_end(){
                body.push(self.parse_statement());
            }
            self.consume(TokenKind::RBrace, "Expected '}' after function body");
            return Stmt::Functions(name, params, body);
        }

        if self.match_token(TokenKind::Return) {
            let expr = self.parse_expression();
            self.consume(TokenKind::Semicolon, "Expected ';' after return value");
            return Stmt::Return(expr);
        }

        if self.match_token(TokenKind::If) {
            return self.parse_if();
        }

        if self.match_token(TokenKind::While) {
            let condition = self.parse_expression();
            self.consume(TokenKind::LBrace, "Expected '{' after while condition");
            let mut body = Vec::new();
            while !self.check_token(TokenKind::RBrace) && !self.is_at_end() {
                body.push(self.parse_statement());
            }
            self.consume(TokenKind::RBrace, "Expected '}' after while body");
            return Stmt::While { condition, body };
        }

        if self.match_token(TokenKind::For) {
            let item_token = self.advance().clone();
            let item_name = match item_token {
                TokenKind::Identifier(n) => n,
                _ => panic!("Expected variable name after for"),
            };

            self.consume(TokenKind::In, "Expected 'in' after for variable");
            let iterable = self.parse_expression();

            self.consume(TokenKind::LBrace, "Expected '{' before for body");
            let mut body = Vec::new();
            while !self.check_token(TokenKind::RBrace) && !self.is_at_end() {
                body.push(self.parse_statement());
            }
            self.consume(TokenKind::RBrace,"Expected '}' after for body");
            return Stmt::For { item: item_name, iterable, body };
        }
        

        let expr = self.parse_expression();
        self.consume(TokenKind::Semicolon,"Expected ';' after expression statement");
        return Stmt::Expr(expr);
    }
    

    fn parse_expression(&mut self) -> Expr {
        let mut left = self.parse_term();
        
        while self.match_token(TokenKind::Plus) 
            || self.match_token(TokenKind::Sub)
            || self.match_token(TokenKind::Equal)
            || self.match_token(TokenKind::Less)
            || self.match_token(TokenKind::Greater)
        {
            let op_token = self.previous_token().clone();
            let right = self.parse_term();
            let op = match op_token {
                TokenKind::Plus => BinaryOp::Add,
                TokenKind::Sub => BinaryOp::Sub,
                TokenKind::Equal => BinaryOp::Equal,
                TokenKind::Less => BinaryOp::Less,
                TokenKind::Greater => BinaryOp::Greater,
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
       
        if self.match_token(TokenKind::LBracket) {
            let mut elements = Vec::new();
            if !self.check_token(TokenKind::RBracket) {
                loop {
                    elements.push(self.parse_expression());
                    if !self.match_token(TokenKind::Comma) { break; }
                }
            }
            self.consume(TokenKind::RBracket, "Expected ']' after list elements");
            return Expr::List(elements);
        }

        let token = self.advance().clone();
        let mut expr = match token {
            TokenKind::Str(s) => Expr::Literal(Value::Str(Rc::new(s))),
            TokenKind::FStr(s) => Self::parse_interpolated_string(&s),
            TokenKind::Int(n) => Expr::Literal(Value::Int(n)),
            TokenKind::Float(n) => Expr::Literal(Value::Float(n)),
            TokenKind::True => Expr::Literal(Value::Bool(true)),
            TokenKind::False => Expr::Literal(Value::Bool(false)),
            TokenKind::Identifier(name) => Expr::Variable(name),
            _ => panic!("[Line {}] Parser: expect expression, got {:?}", self.current_line(), token),
        };
        loop {
            if self.match_token(TokenKind::LParen) {
                let mut args = Vec::new();
                if !self.check_token(TokenKind::RParen) {
                    loop {
                        args.push(self.parse_expression());
                        if !self.match_token(TokenKind::Comma) { break; }
                    }
                }
                self.consume(TokenKind::RParen, "Expected ')' after arguments");
                expr = Expr::Call(Box::new(expr), args);
            } else if self.match_token(TokenKind::LBracket) {
                let index = self.parse_expression();
                self.consume(TokenKind::RBracket, "Expected ']' after index");
                expr = Expr::Index(Box::new(expr), Box::new(index));
            } else {
                break; // Нет ни `(`, ни `[`
            }
        }
        
        expr
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

    fn parse_if(&mut self) -> Stmt {
            let condition = self.parse_expression();

            self.consume(TokenKind::LBrace, "Expected '{' after if condition");
            let mut then_branch = Vec::new();
            while !self.check_token(TokenKind::RBrace) && !self.is_at_end() {
                then_branch.push(self.parse_statement());
            }
            self.consume(TokenKind::RBrace, "Expected '}' after if block");

            let else_branch = if self.match_token(TokenKind::Else) {

                if self.match_token(TokenKind::If){
                    Some(vec![self.parse_if()])
                } else {
                    self.consume(TokenKind::LBrace, "Expected '{' after else");
                    let mut branch = Vec::new();
                    
                    while !self.check_token(TokenKind::RBrace) && !self.is_at_end() {
                        branch.push(self.parse_statement());
                    }
                    
                    self.consume(TokenKind::RBrace, "Expected '}' after else block");
                    Some(branch)
                }

                
            } else {
                None
            };

            return Stmt::If { condition, then_branch, else_branch }

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
