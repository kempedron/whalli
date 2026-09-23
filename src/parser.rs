use std::sync::Arc;

use crate::{
    ast::{BinaryOp, Expr, MatchArm, Pattern, SelectArm, SelectArmKind, Stmt, UnaryOp},
    lexer::{Lexer, Token, TokenKind},
    value::Value,
};

#[derive(Debug)]
pub struct ParseError {
    pub message: String,
    pub line: usize,
}

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    fn error(&self, message: &str) -> ParseError {
        ParseError {
            message: message.to_string(),
            line: self.current_line(),
        }
    }

    pub fn parse(&mut self) -> Result<Vec<Stmt>, ParseError> {
        let mut statements = Vec::new();
        while !self.is_at_end() {
            if self.match_token(TokenKind::NewLine) || self.match_token(TokenKind::Semicolon) {
                continue;
            }
            statements.push(Stmt::Line(self.current_line()));
            statements.push(self.parse_statement()?);
        }
        Ok(statements)
    }

    fn consume_stmt_end(&mut self) -> Result<(), ParseError> {
        if self.is_at_end() || self.check_token(TokenKind::RBrace) {
            return Ok(());
        }
        if self.match_token(TokenKind::Semicolon) || self.match_token(TokenKind::NewLine) {
            while self.match_token(TokenKind::Semicolon) || self.match_token(TokenKind::NewLine) {}
            return Ok(());
        }
        Err(self.error("Expected newline or ';' at the end of statement"))
    }

    fn parse_block(&mut self) -> Result<Vec<Stmt>, ParseError> {
        self.consume(TokenKind::LBrace, "Expected '{' before block")?;
        let mut stmts = Vec::new();
        while !self.check_token(TokenKind::RBrace) && !self.is_at_end() {
            if self.match_token(TokenKind::NewLine) || self.match_token(TokenKind::Semicolon) {
                continue;
            }
            stmts.push(Stmt::Line(self.current_line()));
            stmts.push(self.parse_statement()?);
        }
        self.consume(TokenKind::RBrace, "Expected '}' after block")?;
        Ok(stmts)
    }

    fn current_line(&self) -> usize {
        if self.pos < self.tokens.len() {
            self.tokens[self.pos].line
        } else {
            self.tokens.last().map_or(1, |t| t.line)
        }
    }

    fn parse_statement(&mut self) -> Result<Stmt, ParseError> {
        if self.check_token(TokenKind::LBrace) {
            return Ok(Stmt::Block(self.parse_block()?));
        }

        if self.match_token(TokenKind::Function) {
            return self.parse_function();
        }

        if self.match_token(TokenKind::Struct) {
            let name_token = self.advance().clone();
            let struct_name = match name_token {
                TokenKind::Identifier(n) => n,
                _ => Err(self.error("Expected struct name"))?,
            };

            self.consume(TokenKind::LBrace, "Expected '{' after struct name")?;

            let mut fields = Vec::new();
            while !self.check_token(TokenKind::RBrace) && !self.is_at_end() {
                while self.match_token(TokenKind::NewLine) {}
                if self.check_token(TokenKind::RBrace) {
                    break;
                }

                let field_token = self.advance().clone();
                let field_name = match field_token {
                    TokenKind::Identifier(n) => n,
                    _ => Err(self.error("Expected field name"))?,
                };
                self.consume(TokenKind::Colon, "Expected ':' after field name")?;

                let token_type = self.advance().clone();
                let type_name = match token_type {
                    TokenKind::Identifier(n) => n,
                    _ => Err(self.error("Expected type name"))?,
                };

                fields.push((field_name, type_name));

                while self.match_token(TokenKind::NewLine) || self.match_token(TokenKind::Comma) {}
            }
            self.consume(TokenKind::RBrace, "Expected '}' after struct body")?;
            return Ok(Stmt::Struct(struct_name, fields));
        }

        if self.match_token(TokenKind::Impl) {
            let name_token = self.advance().clone();
            let target_name = match name_token {
                TokenKind::Identifier(n) => n,
                _ => return Err(self.error("Expected struct name after 'impl'")),
            };
            self.consume(TokenKind::LBrace, "Expected '{' after struct name")?;

            let mut methods = Vec::new();
            while !self.check_token(TokenKind::RBrace) && !self.is_at_end() {
                while self.match_token(TokenKind::NewLine) {}
                if self.check_token(TokenKind::RBrace) {
                    break;
                }

                if self.match_token(TokenKind::Function) {
                    methods.push(self.parse_function()?);
                } else {
                    return Err(self.error("Only functions are allowed inside 'impl' blocks"));
                }
            }
            self.consume(TokenKind::RBrace, "Expected '}' after impl block")?;
            return Ok(Stmt::Impl(target_name, methods));
        }

        if self.match_token(TokenKind::Interface) {
            let name_token = self.advance().clone();
            let interface_name = match name_token {
                TokenKind::Identifier(n) => n,
                _ => return Err(self.error("Expected interface name")),
            };
            self.consume(TokenKind::LBrace, "Expected '{' after interface name")?;

            let mut methods = Vec::new();
            while !self.check_token(TokenKind::RBrace) && !self.is_at_end() {
                while self.match_token(TokenKind::NewLine) {}
                if self.check_token(TokenKind::RBrace) {
                    break;
                }

                self.consume(TokenKind::Function, "Expected 'func' in interface body")?;

                let method_token = self.advance().clone();
                match method_token {
                    TokenKind::Identifier(n) => methods.push(n),
                    _ => return Err(self.error("Expected method name in interface")),
                }

                if self.match_token(TokenKind::LParen) {
                    self.consume(TokenKind::RParen, "Expected ')' after '(' in interface")?;
                }

                while self.match_token(TokenKind::NewLine) || self.match_token(TokenKind::Comma) {}
            }
            self.consume(TokenKind::RBrace, "Expected '}' after interface body")?;
            return Ok(Stmt::Interface(interface_name, methods));
        }

        if self.match_token(TokenKind::Let) {
            if self.match_token(TokenKind::LParen) {
                let mut names = Vec::new();
                if !self.check_token(TokenKind::RParen) {
                    loop {
                        let name_token = self.advance().clone();
                        match name_token {
                            TokenKind::Identifier(n) => names.push(n),
                            _ => {
                                return Err(
                                    self.error("Expected variable name in tuple destructuring")
                                );
                            }
                        }
                        if !self.match_token(TokenKind::Comma) {
                            break;
                        }
                    }
                }
                self.consume(TokenKind::RParen, "Expected ')' after tuple variables")?;
                self.consume(TokenKind::Assign, "Expected '=' after tuple variables")?;
                let expr = self.parse_expression()?;
                return Ok(Stmt::LetTuple(names, expr));
            } else {
                let name_token = self.advance().clone();
                let name = match name_token {
                    TokenKind::Identifier(n) => n,
                    _ => return Err(self.error("Expect variable name")),
                };
                self.consume(TokenKind::Assign, "Expect '=' after variable name")?;
                let expr = self.parse_expression()?;
                return Ok(Stmt::Let(name, expr));
            }
        }

        if self.match_token(TokenKind::Return) {
            let expr = if self.check_token(TokenKind::NewLine)
                || self.check_token(TokenKind::Semicolon)
                || self.check_token(TokenKind::RBrace)
                || self.is_at_end()
            {
                Expr::Literal(Value::Nil)
            } else {
                self.parse_expression()?
            };

            self.consume_stmt_end()?;
            return Ok(Stmt::Return(expr));
        }

        if self.match_token(TokenKind::If) {
            return self.parse_if();
        }

        if self.match_token(TokenKind::While) {
            let condition = self.parse_expression()?;
            let body = self.parse_block()?;
            return Ok(Stmt::While { condition, body });
        }

        if self.match_token(TokenKind::For) {
            let item_token = self.advance().clone();
            let item_name = match item_token {
                TokenKind::Identifier(n) => n,
                _ => return Err(self.error("Expected variable name after for")),
            };

            self.consume(TokenKind::In, "Expected 'in' after for variable")?;
            let iterable = self.parse_expression()?;

            let body = self.parse_block()?;
            return Ok(Stmt::For {
                item: item_name,
                iterable,
                body,
            });
        }

        if self.match_token(TokenKind::Break) {
            self.consume_stmt_end()?;
            return Ok(Stmt::Break);
        }

        if self.match_token(TokenKind::Continue) {
            self.consume_stmt_end()?;
            return Ok(Stmt::Continue);
        }

        if self.match_token(TokenKind::Import) {
            let name_token = self.advance().clone();
            match name_token {
                // import "./path/to/file.wh"  — local file import
                TokenKind::Str(path) => {
                    self.consume_stmt_end()?;
                    return Ok(Stmt::ImportFile(path));
                }
                // import net  — stdlib module import
                TokenKind::Identifier(n) => {
                    self.consume_stmt_end()?;
                    return Ok(Stmt::Import(n));
                }
                _ => return Err(self.error("Expected module name or file path string after 'import'")),
            }
        }

        if self.match_token(TokenKind::Wo) {
            let expr = self.parse_primary()?;
            self.consume_stmt_end()?;

            if let Expr::Call(callee, args) = expr {
                return Ok(Stmt::Spawn(callee, args));
            } else {
                return Err(self.error("Expected function call after 'wo'"));
            }
        }

        if self.match_token(TokenKind::Defer) {
            let expr = self.parse_expression()?;
            self.consume_stmt_end()?;

            match expr {
                Expr::Call(_, _) | Expr::MethodCall(_, _, _) => {
                    return Ok(Stmt::Defer(Box::new(expr)));
                }
                _ => return Err(self.error("Expected function or method call after 'defer'")),
            }
        }

        let expr = self.parse_expression()?;

        if self.match_token(TokenKind::Assign) {
            let value = self.parse_expression()?;
            self.consume_stmt_end()?;

            return match expr {
                Expr::Variable(name) => Ok(Stmt::Assign(name, value)),
                Expr::Index(array, idx) => Ok(Stmt::IndexAssign(*array, *idx, value)),
                _ => Err(self.error("Invalid assignment target")),
            };
        }

        let op = if self.match_token(TokenKind::PlusAssign) {
            Some(BinaryOp::Add)
        } else if self.match_token(TokenKind::SubAssign) {
            Some(BinaryOp::Sub)
        } else if self.match_token(TokenKind::MulAssign) {
            Some(BinaryOp::Mul)
        } else if self.match_token(TokenKind::DivAssign) {
            Some(BinaryOp::Div)
        } else if self.match_token(TokenKind::ModAssign) {
            Some(BinaryOp::Mod)
        } else {
            None
        };

        if let Some(binary_op) = op {
            let value = self.parse_expression()?;
            self.consume_stmt_end()?;

            return match expr.clone() {
                Expr::Variable(name) => {
                    let new_expr = Expr::Binary(Box::new(expr), binary_op, Box::new(value));
                    Ok(Stmt::Assign(name, new_expr))
                }
                Expr::Index(arr, idx) => {
                    let new_expr = Expr::Binary(Box::new(expr), binary_op, Box::new(value));
                    Ok(Stmt::IndexAssign(*arr, *idx, new_expr))
                }
                _ => Err(self.error("Invalid assignment target")),
            };
        }

        self.consume_stmt_end()?;
        Ok(Stmt::Expr(expr))
    }

    fn parse_expression(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_or()?;
        if self.match_token(TokenKind::LArrow) {
            let value = self.parse_expression()?;
            expr = Expr::ChanSend(Box::new(expr), Box::new(value));
        }
        Ok(expr)
    }

    fn parse_function(&mut self) -> Result<Stmt, ParseError> {
        let name_token = self.advance().clone();
        let name = match name_token {
            TokenKind::Identifier(n) => n,
            _ => return Err(self.error("Expect function name after 'func'")),
        };

        self.consume(TokenKind::LParen, "Expect '(' after function name")?;

        let mut params: Vec<(String, Option<String>)> = Vec::new();
        if !self.check_token(TokenKind::RParen) {
            loop {
                let param_token = self.advance().clone();
                let param_name = match param_token {
                    TokenKind::Identifier(n) => n,
                    _ => return Err(self.error("Expect parameter name")),
                };
                let mut param_type = None;
                if self.match_token(TokenKind::Colon) {
                    param_type = Some(self.parse_type_name()?);
                }

                params.push((param_name, param_type));

                if !self.match_token(TokenKind::Comma) {
                    break;
                }
            }
        }
        self.consume(TokenKind::RParen, "Expected ')' after parameters")?;

        let mut return_type = None;
        if self.match_token(TokenKind::Arrow) {
            return_type = Some(self.parse_type_name()?);
        }

        let body = self.parse_block()?;
        Ok(Stmt::Functions(name, params, return_type, body))
    }

    fn parse_or(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_and()?;
        while self.match_token(TokenKind::Or) {
            let right = self.parse_and()?;
            left = Expr::Binary(Box::new(left), BinaryOp::Or, Box::new(right));
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_equality()?;
        while self.match_token(TokenKind::And) {
            let right = self.parse_equality()?;
            left = Expr::Binary(Box::new(left), BinaryOp::And, Box::new(right))
        }
        Ok(left)
    }

    fn parse_equality(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_term()?;

        while self.match_token(TokenKind::Plus)
            || self.match_token(TokenKind::Sub)
            || self.match_token(TokenKind::Equal)
            || self.match_token(TokenKind::BangEqual)
            || self.match_token(TokenKind::Less)
            || self.match_token(TokenKind::Greater)
            || self.match_token(TokenKind::LessEqual)
            || self.match_token(TokenKind::GreaterEqual)
            || self.match_token(TokenKind::Is)
        {
            let op_token = self.previous_token().clone();
            let right = self.parse_term()?;

            if op_token == TokenKind::Is {
                left = Expr::Is(Box::new(left), Box::new(right));
            } else {
                let op = match op_token {
                    TokenKind::Plus => BinaryOp::Add,
                    TokenKind::Sub => BinaryOp::Sub,
                    TokenKind::Equal => BinaryOp::Equal,
                    TokenKind::BangEqual => BinaryOp::NotEqual,
                    TokenKind::Less => BinaryOp::Less,
                    TokenKind::Greater => BinaryOp::Greater,
                    TokenKind::LessEqual => BinaryOp::LessEqual,
                    TokenKind::GreaterEqual => BinaryOp::GreaterEqual,
                    _ => unreachable!(),
                };
                left = Expr::Binary(Box::new(left), op, Box::new(right));
            }
        }
        Ok(left)
    }

    fn parse_term(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_primary()?;

        while self.match_token(TokenKind::Mul)
            || self.match_token(TokenKind::Div)
            || self.match_token(TokenKind::Mod)
        {
            let op_token = self.previous_token().clone();
            let right = self.parse_primary()?;
            let op = match op_token {
                TokenKind::Mul => BinaryOp::Mul,
                TokenKind::Div => BinaryOp::Div,
                TokenKind::Mod => BinaryOp::Mod,
                _ => unreachable!(),
            };
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        if self.match_token(TokenKind::Match) {
            return self.parse_match();
        }

        if self.match_token(TokenKind::Select) {
            return self.parse_select();
        }

        if self.match_token(TokenKind::Sub) {
            let expr = self.parse_primary()?;
            match expr {
                Expr::Literal(Value::Int(n)) => return Ok(Expr::Literal(Value::Int(-n))),
                Expr::Literal(Value::Float(f)) => return Ok(Expr::Literal(Value::Float(-f))),
                other => {
                    return Ok(Expr::Binary(
                        Box::new(Expr::Literal(Value::Int(0))),
                        BinaryOp::Sub,
                        Box::new(other),
                    ));
                }
            }
        }

        if self.match_token(TokenKind::Not) {
            let expr = self.parse_primary()?;
            return Ok(Expr::Unary(UnaryOp::Not, Box::new(expr)));
        }

        if self.match_token(TokenKind::LArrow) {
            let right = self.parse_primary()?;
            return Ok(Expr::ChanRecv(Box::new(right)));
        }

        if self.match_token(TokenKind::Wo) {
            let expr = self.parse_primary()?;
            if let Expr::Call(callee, args) = expr {
                return Ok(Expr::Spawn(callee, args));
            } else {
                return Err(self.error("Expected function call after 'wo'"));
            }
        }

        if self.match_token(TokenKind::LParen) {
            if self.match_token(TokenKind::RParen) {
                return Ok(Expr::Tuple(Vec::new()));
            }
            let expr = self.parse_expression()?;

            if self.match_token(TokenKind::Comma) {
                let mut elements = vec![expr];
                if !self.check_token(TokenKind::RParen) {
                    loop {
                        elements.push(self.parse_expression()?);
                        if !self.match_token(TokenKind::Comma) {
                            break;
                        }
                    }
                }
                self.consume(TokenKind::RParen, "Expect closing bracket ')'")?;
                return Ok(Expr::Tuple(elements));
            }

            self.consume(TokenKind::RParen, "Expect closing bracket ')'")?;
            return Ok(expr);
        }

        if self.match_token(TokenKind::LBracket) {
            let mut elements = Vec::new();
            if !self.check_token(TokenKind::RBracket) {
                loop {
                    elements.push(self.parse_expression()?);
                    if !self.match_token(TokenKind::Comma) {
                        break;
                    }
                }
            }
            self.consume(TokenKind::RBracket, "Expected ']' after list elements")?;
            return Ok(Expr::List(elements));
        }

        if self.match_token(TokenKind::LBrace) {
            let mut entries = Vec::new();
            if !self.check_token(TokenKind::RBrace) {
                loop {
                    while self.match_token(TokenKind::NewLine) {}
                    if self.check_token(TokenKind::RBrace) {
                        break;
                    }

                    let key = self.parse_expression()?;
                    self.consume(TokenKind::Colon, "Expected ':' after map key")?;

                    while self.match_token(TokenKind::NewLine) {}

                    let val = self.parse_expression()?;
                    entries.push((key, val));

                    while self.match_token(TokenKind::NewLine) {}
                    if !self.match_token(TokenKind::Comma) {
                        break;
                    }
                }
            }
            while self.match_token(TokenKind::NewLine) {}
            self.consume(TokenKind::RBrace, "Expected '}' after map")?;
            return Ok(Expr::Map(entries));
        }

        let token = self.advance().clone();
        let mut expr = match token {
            TokenKind::Str(s) => Expr::Literal(Value::Str(Arc::new(s))),
            TokenKind::FStr(s) => self.parse_interpolated_string(&s)?,
            TokenKind::Int(n) => Expr::Literal(Value::Int(n)),
            TokenKind::Float(n) => Expr::Literal(Value::Float(n)),
            TokenKind::True => Expr::Literal(Value::Bool(true)),
            TokenKind::False => Expr::Literal(Value::Bool(false)),
            TokenKind::Nil => Expr::Literal(Value::Nil),
            TokenKind::Identifier(name) => Expr::Variable(name),
            _ => return Err(self.error(&format!("Expect expression, got {:?}", token))),
        };

        loop {
            if self.match_token(TokenKind::LParen) {
                let mut args = Vec::new();
                if !self.check_token(TokenKind::RParen) {
                    loop {
                        args.push(self.parse_expression()?);
                        if !self.match_token(TokenKind::Comma) {
                            break;
                        }
                    }
                }
                self.consume(TokenKind::RParen, "Expected ')' after arguments")?;
                expr = Expr::Call(Box::new(expr), args);
            } else if self.match_token(TokenKind::LBracket) {
                let index = self.parse_expression()?;
                self.consume(TokenKind::RBracket, "Expected ']' after index")?;
                expr = Expr::Index(Box::new(expr), Box::new(index));
            } else if self.match_token(TokenKind::Dot) {
                let name_token = self.advance().clone();
                let prop_name = match name_token {
                    TokenKind::Identifier(n) => n,
                    _ => return Err(self.error("Expected property name after '.'")),
                };
                if self.match_token(TokenKind::LParen) {
                    let mut args = Vec::new();
                    if !self.check_token(TokenKind::RParen) {
                        loop {
                            args.push(self.parse_expression()?);
                            if !self.match_token(TokenKind::Comma) {
                                break;
                            }
                        }
                    }
                    self.consume(TokenKind::RParen, "Expected ')' after arguments")?;
                    expr = Expr::MethodCall(Box::new(expr), prop_name, args);
                } else {
                    let string_key = Expr::Literal(Value::Str(Arc::new(prop_name)));
                    expr = Expr::Index(Box::new(expr), Box::new(string_key));
                }
            } else if self.match_token(TokenKind::Question) {
                expr = Expr::Try(Box::new(expr));
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn parse_interpolated_string(&self, s: &str) -> Result<Expr, ParseError> {
        let mut parts: Vec<Expr> = Vec::new();
        let chars: Vec<char> = s.chars().collect();
        let mut i = 0;

        let mut current_literal = String::new();

        while i < chars.len() {
            if chars[i] == '{' {
                if !current_literal.is_empty() {
                    parts.push(Expr::Literal(Value::Str(Arc::new(current_literal))));
                    current_literal = String::new();
                }
                i += 1;
                let mut expr_str = String::new();
                while i < chars.len() && chars[i] != '}' {
                    expr_str.push(chars[i]);
                    i += 1;
                }
                if i < chars.len() {
                    i += 1;
                }

                let code_inside = expr_str.trim();
                if !code_inside.is_empty() {
                    let mut sub_lexer = Lexer::new(code_inside);
                    let tokens = sub_lexer.tokenize().map_err(|e| ParseError {
                        message: format!("In f-string: {}", e.message),
                        line: self.current_line(),
                    })?;
                    let mut sub_parser = Parser::new(tokens);

                    let expr = sub_parser.parse_expression().map_err(|e| ParseError {
                        message: format!("In f-string: {}", e.message),
                        line: self.current_line(),
                    })?;
                    parts.push(expr);
                }
            } else {
                current_literal.push(chars[i]);
                i += 1
            }
        }
        if !current_literal.is_empty() {
            parts.push(Expr::Literal(Value::Str(Arc::new(current_literal))));
        }

        if parts.is_empty() {
            return Ok(Expr::Literal(Value::Str(Arc::new("".to_string()))));
        }

        let mut expr = parts[0].clone();
        for next_part in parts.into_iter().skip(1) {
            expr = Expr::Binary(Box::new(expr), BinaryOp::Add, Box::new(next_part));
        }
        Ok(expr)
    }

    fn parse_if(&mut self) -> Result<Stmt, ParseError> {
        let condition = self.parse_expression()?;
        let then_branch = self.parse_block()?;

        let else_branch = if self.match_token(TokenKind::Else) {
            if self.match_token(TokenKind::If) {
                Some(vec![self.parse_if()?])
            } else {
                Some(self.parse_block()?)
            }
        } else {
            None
        };

        Ok(Stmt::If {
            condition,
            then_branch,
            else_branch,
        })
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
        if !self.is_at_end() {
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

    fn consume(&mut self, expected: TokenKind, err_msg: &str) -> Result<(), ParseError> {
        if self.match_token(expected) {
            return Ok(());
        }
        Err(self.error(err_msg))
    }

    fn check_token(&self, expected: TokenKind) -> bool {
        if self.is_at_end() {
            return false;
        }
        *self.current_token() == expected
    }

    fn parse_type_name(&mut self) -> Result<String, ParseError> {
        if self.match_token(TokenKind::LParen) {
            if self.match_token(TokenKind::RParen) {
                return Ok("()".to_string());
            }
            let mut types = Vec::new();
            loop {
                types.push(self.parse_type_name()?);
                if !self.match_token(TokenKind::Comma) {
                    break;
                }
            }
            self.consume(TokenKind::RParen, "Expected ')' after tuple type")?;
            Ok(format!("({})", types.join(", ")))
        } else {
            let token = self.advance().clone();
            match token {
                TokenKind::Identifier(n) => Ok(n),
                _ => Err(self.error("Expected type name")),
            }
        }
    }

    fn parse_match(&mut self) -> Result<Expr, ParseError> {
        let subject = self.parse_expression()?;
        while self.match_token(TokenKind::NewLine) {}
        self.consume(TokenKind::LBrace, "Expected '{' after match expression")?;

        let mut arms = Vec::new();
        while !self.check_token(TokenKind::RBrace) && !self.is_at_end() {
            while self.match_token(TokenKind::NewLine) || self.match_token(TokenKind::Semicolon) {}
            if self.check_token(TokenKind::RBrace) {
                break;
            }
            arms.push(self.parse_match_arm()?);
            while self.match_token(TokenKind::Comma) || self.match_token(TokenKind::NewLine) || self.match_token(TokenKind::Semicolon) {}
        }
        self.consume(TokenKind::RBrace, "Expected '}' after match block")?;

        Ok(Expr::Match {
            subject: Box::new(subject),
            arms,
        })
    }

    fn parse_match_arm(&mut self) -> Result<MatchArm, ParseError> {
        let pattern = self.parse_pattern()?;
        let guard = if self.match_token(TokenKind::If) {
            Some(self.parse_expression()?)
        } else {
            None
        };

        self.consume(TokenKind::FatArrow, "Expected '=>' after pattern")?;
        while self.match_token(TokenKind::NewLine) {}

        let body = self.parse_expression()?;

        Ok(MatchArm {
            pattern,
            guard,
            body,
        })
    }

    fn parse_pattern(&mut self) -> Result<Pattern, ParseError> {
        let first = self.parse_single_pattern()?;
        if self.check_token(TokenKind::Pipe) {
            let mut alternatives = vec![first];
            while self.match_token(TokenKind::Pipe) {
                alternatives.push(self.parse_single_pattern()?);
            }
            Ok(Pattern::Or(alternatives))
        } else {
            Ok(first)
        }
    }

    fn parse_single_pattern(&mut self) -> Result<Pattern, ParseError> {
        while self.match_token(TokenKind::NewLine) {}

        if self.match_token(TokenKind::LParen) {
            let mut elements = Vec::new();
            if !self.check_token(TokenKind::RParen) {
                loop {
                    elements.push(self.parse_pattern()?);
                    if !self.match_token(TokenKind::Comma) {
                        break;
                    }
                }
            }
            self.consume(TokenKind::RParen, "Expected ')' after tuple pattern")?;
            return Ok(Pattern::Tuple(elements));
        }

        let token = self.advance().clone();
        match token {
            TokenKind::Identifier(name) => {
                if name == "_" {
                    Ok(Pattern::Wildcard)
                } else if self.match_token(TokenKind::Colon) {
                    let type_name = self.parse_type_name()?;
                    Ok(Pattern::Type(name, type_name))
                } else {
                    Ok(Pattern::Variable(name))
                }
            }
            TokenKind::Int(start) => {
                if self.match_token(TokenKind::DotDotEqual) {
                    let end_token = self.advance().clone();
                    if let TokenKind::Int(end) = end_token {
                        Ok(Pattern::Range { start, end, inclusive: true })
                    } else {
                        Err(self.error("Expected integer after '..=' in pattern"))
                    }
                } else if self.match_token(TokenKind::DotDot) {
                    let end_token = self.advance().clone();
                    if let TokenKind::Int(end) = end_token {
                        Ok(Pattern::Range { start, end, inclusive: false })
                    } else {
                        Err(self.error("Expected integer after '..' in pattern"))
                    }
                } else {
                    Ok(Pattern::Literal(Value::Int(start)))
                }
            }
            TokenKind::Float(f) => Ok(Pattern::Literal(Value::Float(f))),
            TokenKind::Str(s) => Ok(Pattern::Literal(Value::Str(Arc::new(s)))),
            TokenKind::True => Ok(Pattern::Literal(Value::Bool(true))),
            TokenKind::False => Ok(Pattern::Literal(Value::Bool(false))),
            TokenKind::Nil => Ok(Pattern::Literal(Value::Nil)),
            other => Err(self.error(&format!("Expected pattern, found {:?}", other))),
        }
    }

    fn parse_select(&mut self) -> Result<Expr, ParseError> {
        while self.match_token(TokenKind::NewLine) {}
        self.consume(TokenKind::LBrace, "Expected '{' after 'select'")?;

        let mut arms = Vec::new();
        while !self.check_token(TokenKind::RBrace) && !self.is_at_end() {
            while self.match_token(TokenKind::NewLine) || self.match_token(TokenKind::Semicolon) {}
            if self.check_token(TokenKind::RBrace) {
                break;
            }

            let arm = self.parse_select_arm()?;
            arms.push(arm);

            while self.match_token(TokenKind::Comma)
                || self.match_token(TokenKind::NewLine)
                || self.match_token(TokenKind::Semicolon)
            {}
        }
        self.consume(TokenKind::RBrace, "Expected '}' after select block")?;

        Ok(Expr::Select(arms))
    }

    fn parse_select_arm(&mut self) -> Result<SelectArm, ParseError> {
        if self.match_token(TokenKind::Default) {
            self.consume(TokenKind::FatArrow, "Expected '=>' after 'default'")?;
            while self.match_token(TokenKind::NewLine) {}
            let body = self.parse_expression()?;
            return Ok(SelectArm {
                kind: SelectArmKind::Default,
                body,
            });
        }

        // <- channel_expr => body (anonymous receive, e.g. <- time.after(1.0) => ...)
        if self.match_token(TokenKind::LArrow) {
            let chan_expr = self.parse_expression()?;
            self.consume(TokenKind::FatArrow, "Expected '=>' after select case")?;
            while self.match_token(TokenKind::NewLine) {}
            let body = self.parse_expression()?;
            return Ok(SelectArm {
                kind: SelectArmKind::Recv("_".to_string(), chan_expr),
                body,
            });
        }

        // identifier <- channel_expr => body (receive)
        if self.pos + 1 < self.tokens.len() && self.tokens[self.pos + 1].kind == TokenKind::LArrow {
            let ident_token = self.advance().clone();
            let var_name = match ident_token {
                TokenKind::Identifier(name) => name,
                _ => return Err(self.error("Expected variable name before '<-' in select")),
            };
            self.consume(TokenKind::LArrow, "Expected '<-' after variable in select")?;
            let chan_expr = self.parse_expression()?;
            self.consume(TokenKind::FatArrow, "Expected '=>' after select case")?;
            while self.match_token(TokenKind::NewLine) {}
            let body = self.parse_expression()?;
            return Ok(SelectArm {
                kind: SelectArmKind::Recv(var_name, chan_expr),
                body,
            });
        }

        // channel_expr <- val_expr => body (send)
        let expr = self.parse_expression()?;
        if let Expr::ChanSend(chan, val) = expr {
            self.consume(TokenKind::FatArrow, "Expected '=>' after send expression in select")?;
            while self.match_token(TokenKind::NewLine) {}
            let body = self.parse_expression()?;
            return Ok(SelectArm {
                kind: SelectArmKind::Send(*chan, *val),
                body,
            });
        }

        Err(self.error("Expected select arm: 'var <- chan => ...', 'chan <- val => ...', or 'default => ...'"))
    }
}
