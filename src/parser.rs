use std::rc::Rc;

use crate::{ast::{BinaryOp, Expr, Stmt, UnaryOp}, value::Value, lexer::{Token, TokenKind, Lexer}};

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
            if self.match_token(TokenKind::NewLine) || self.match_token(TokenKind::Semicolon){
                continue;
            }
            statements.push(self.parse_statement());
        }
        statements
    }

    fn consume_stmt_end(&mut self) {
        if self.is_at_end() || self.check_token(TokenKind::RBrace) {
            return;
        }
        if self.match_token(TokenKind::Semicolon) || self.match_token(TokenKind::NewLine) {
            while self.match_token(TokenKind::Semicolon) || self.match_token(TokenKind::NewLine) {}
            return;
        }
        panic!("[Line {}] Parser: Expected newline or ';' at the end of statement", self.current_line());
    }

    fn parse_block(&mut self) -> Vec<Stmt> {
        self.consume(TokenKind::LBrace, "Expected '{' before block");
        let mut stmts = Vec::new();
        while !self.check_token(TokenKind::RBrace) && !self.is_at_end() {
            if self.match_token(TokenKind::NewLine) || self.match_token(TokenKind::Semicolon) {
                continue;
            }
            stmts.push(self.parse_statement());
        }
        self.consume(TokenKind::RBrace, "Expected '}' after block");
        stmts
    }

    fn  current_line(&self) -> usize {
        self.tokens[self.pos].line
    }

    fn parse_statement(&mut self) -> Stmt {

        if self.check_token(TokenKind::LBrace) {
            return Stmt::Block(self.parse_block());
        }

        if self.match_token(TokenKind::Let){
            let name_token = self.advance().clone();
            let name = match name_token {
                TokenKind::Identifier(n) => n,
                _ => panic!("[Line {}] Expect var name after 'let'",self.current_line())
            };
            self.consume(TokenKind::Assign, "Parser: Expected '=' sign after variable name");
            let expr = self.parse_expression();
            self.consume_stmt_end();


            return Stmt::Let(name, expr);
        }

        if self.match_token(TokenKind::Function) {
            let name_token = self.advance().clone();
            let name = match name_token{
                TokenKind::Identifier(n) => n,
                _ => panic!("[Line {}] Expect function name after 'func'", self.current_line()),
            };

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

            let body = self.parse_block();
            return Stmt::Functions(name, params, body);
        }

        if self.match_token(TokenKind::Return) {
            let expr = self.parse_expression();
            self.consume_stmt_end();
            return Stmt::Return(expr);
        }

        if self.match_token(TokenKind::If) {
            return self.parse_if();
        }

        if self.match_token(TokenKind::While) {
            let condition = self.parse_expression();
            let body = self.parse_block();
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

            let body = self.parse_block();
            return Stmt::For { item: item_name, iterable, body };
        }
        
        if self.match_token(TokenKind::Break) {
            self.consume_stmt_end();
            return Stmt::Break;
        }

        if self.match_token(TokenKind::Continue) {
            self.consume_stmt_end();
            return Stmt::Continue;
        }

        let expr = self.parse_expression();

        if self.match_token(TokenKind::Assign) {
            let value = self.parse_expression();
            self.consume_stmt_end();

            match expr {
                Expr::Variable(name) => return Stmt::Assign(name, value),
                Expr::Index(array, idx) => return Stmt::IndexAssign(*array, *idx, value),
                _ => panic!("Parse error: Invalid assignment target"),
            }
        }

        let op  = if self.match_token(TokenKind::PlusAssign) {Some(BinaryOp::Add)}
            else if self.match_token(TokenKind::SubAssign) { Some(BinaryOp::Sub) }
            else if self.match_token(TokenKind::MulAssign) {Some(BinaryOp::Mul)}
            else if self.match_token(TokenKind::DivAssign) {Some(BinaryOp::Div)}
            else if self.match_token(TokenKind::ModAssign) {Some(BinaryOp::Mod)}
            else {None};

        if let Some(binary_op) = op {
            let value = self.parse_expression();
            self.consume_stmt_end();

            match expr.clone() {
                Expr::Variable(name) => {
                    let new_expr = Expr::Binary(Box::new(expr), binary_op, Box::new(value));
                    return Stmt::Assign(name, new_expr);
                }
                Expr::Index(arr, idx) => {
                    let new_expr = Expr::Binary(Box::new(expr), binary_op, Box::new(value));
                    return Stmt::IndexAssign(*arr, *idx, new_expr);
                }
                _ => panic!("Parse error: Invalid assignment target"),
            }
        }

        self.consume_stmt_end();
        return Stmt::Expr(expr);
    }
    

    fn parse_expression(&mut self) -> Expr {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Expr {
        let mut left = self.parse_and();
        while self.match_token(TokenKind::Or) {
            let right = self.parse_and();
            left = Expr::Binary(Box::new(left), BinaryOp::Or, Box::new(right)); 
        }
        left
    }

    fn parse_and(&mut self) -> Expr {
        let mut left = self.parse_equality();
        while self.match_token(TokenKind::And) {
            let right = self.parse_equality();
            left = Expr::Binary(Box::new(left), BinaryOp::And, Box::new(right))
        }
        left
    }

    fn parse_equality(&mut self) -> Expr {
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

        while self.match_token(TokenKind::Mul) || self.match_token(TokenKind::Div) || self.match_token(TokenKind::Mod){
            let op_token = self.previous_token().clone();
            let right = self.parse_primary();
            let op = match op_token {
                TokenKind::Mul => BinaryOp::Mul,
                TokenKind::Div => BinaryOp::Div,
                TokenKind::Mod => BinaryOp::Mod,
                _ => unreachable!(),
            };
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        left
    }

    fn parse_primary(&mut self) -> Expr {

        if self.match_token(TokenKind::Not) {
            let expr = self.parse_primary();
            return Expr::Unary(UnaryOp::Not, Box::new(expr));
        }

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

        if self.match_token(TokenKind::LBrace) {
            let mut entries = Vec::new();
            if !self.check_token(TokenKind::RBrace) {
                loop {
                    while self.match_token(TokenKind::NewLine) {}

                    if self.check_token(TokenKind::RBrace) { break; }

                    let key = self.parse_expression();
                    self.consume(TokenKind::Colon, "Expected ':' after map key");
                    
                    while self.match_token(TokenKind::NewLine) {}

                    let val = self.parse_expression();
                    entries.push((key, val));

                    while self.match_token(TokenKind::NewLine) {}
                    if !self.match_token(TokenKind::Comma) { break; }
                }
            }
            while self.match_token(TokenKind::NewLine) {}
            self.consume(TokenKind::RBrace, "Expected '}' after map");
            return Expr::Map(entries);
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
            } else if self.match_token(TokenKind::Dot) {
              let name_token  = self.advance().clone();
              let prop_name = match  name_token {
                  TokenKind::Identifier(n) => n,
                  _ => panic!("[Line {}] Expected property name after .", self.current_line()),
              };
              if self.match_token(TokenKind::LParen) {
                let mut args = Vec::new();
                if !self.check_token(TokenKind::RParen){
                    loop {
                        args.push(self.parse_expression());
                        if !self.match_token(TokenKind::Comma)  { break ;}
                    }
                }
                self.consume(TokenKind::RParen, "Expected ')' after arguments");
                expr = Expr::MethodCall(Box::new(expr), prop_name, args);
              } else {
                    let string_key = Expr::Literal(Value::Str(Rc::new(prop_name)));
                    expr = Expr::Index(Box::new(expr), Box::new(string_key));
              }
            } else {
                break;
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
                let mut expr_str = String::new();
                while i <  chars.len() && chars[i] != '}' {
                    expr_str.push(chars[i]);
                    i += 1;
                }
                if i < chars.len() { i += 1; }

                let code_inside = expr_str.trim();
                if !code_inside.is_empty(){
                    let mut sub_lexer = Lexer::new(code_inside);
                    let tokens = sub_lexer.tokenize();
                    let mut sub_parser = Parser::new(tokens);

                    let expr = sub_parser.parse_expression();
                    parts.push(expr);
                }
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

            let then_branch = self.parse_block();

        let else_branch = if self.match_token(TokenKind::Else) {
            if self.match_token(TokenKind::If){
                Some(vec![self.parse_if()])
            } else {
                Some(self.parse_block())
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
