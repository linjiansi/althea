use crate::token::{Kind, Token};
use alc_diagnostic::{Diagnostic, FileId, Files, Label, Result, Span, Spanned};
use std::{iter::FusedIterator, str::Chars};

#[derive(Clone, Debug)]
pub struct Lexer<'a> {
    file_id: FileId,
    initial_len: usize,
    chars: Chars<'a>,
}

impl<'a> Iterator for Lexer<'a> {
    type Item = Result<Spanned<Token>>;

    fn next(&mut self) -> Option<Self::Item> {
        let lo = self.index();
        let token = match self.chars.next()? {
            c if c.is_whitespace() => {
                self.skip_whitespace();
                return self.next();
            }
            c if c.is_ascii_digit() => Token::new(Kind::NumberLiteral, &self.next_literal(c)),
            c if c.is_alphabetic() || c == '_' => self.next_ident_or_keyword(c),
            '"' => self.next_string_literal()?,
            '!' => match self.nth_char(0) {
                '=' => {
                    self.chars.next()?;
                    Kind::Neq.into()
                }
                _ => Kind::Not.into(),
            },
            '+' => Kind::Plus.into(),
            '-' => Kind::Minus.into(),
            '*' => Kind::Mul.into(),
            '/' => match self.nth_char(0) {
                '/' => {
                    while self.chars.next()? != '\n' {}
                    return self.next();
                }
                '*' => {
                    self.chars.next()?;
                    while self.chars.next()? != '*' || self.chars.next()? != '/' {}
                    return self.next();
                }
                _ => Kind::Div.into(),
            },
            '&' => Kind::And.into(),
            '|' => Kind::Or.into(),
            '^' => Kind::Xor.into(),
            '(' => Kind::LParen.into(),
            ')' => Kind::RParen.into(),
            '<' => match self.nth_char(0) {
                '=' => {
                    self.chars.next()?;
                    Kind::Leq.into()
                }
                '<' => {
                    self.chars.next()?;
                    Kind::LShift.into()
                }
                _ => Kind::LAngle.into(),
            },
            '>' => match self.nth_char(0) {
                '=' => {
                    self.chars.next()?;
                    Kind::Geq.into()
                }
                '>' => {
                    self.chars.next()?;
                    Kind::RShift.into()
                }
                _ => Kind::RAngle.into(),
            },
            '{' => Kind::LCurl.into(),
            '}' => Kind::RCurl.into(),
            '[' => Kind::LSquare.into(),
            ']' => Kind::RSquare.into(),
            ',' => Kind::Comma.into(),
            ';' => Kind::Semi.into(),
            ':' => match self.nth_char(0) {
                ':' => {
                    self.chars.next()?;
                    Kind::Separator.into()
                }
                _ => Kind::Colon.into(),
            },
            '=' => match self.nth_char(0) {
                '=' => {
                    self.chars.next()?;
                    Kind::EqEq.into()
                }
                '>' => {
                    self.chars.next()?;
                    Kind::MatchArrow.into()
                }
                _ => Kind::Eq.into(),
            },
            '.' => Kind::Dot.into(),
            c => {
                return Some(Err(Box::from(Diagnostic::new_error(
                    "found invalid token",
                    Label::new(
                        self.file_id,
                        lo..self.index(),
                        format!("'{}' is not valid here", c),
                    ),
                ))))
            }
        };
        let hi = self.index();
        Some(Ok(Span::new(lo, hi).span(token)))
    }
}

impl<'a> FusedIterator for Lexer<'a> {}

impl<'a> Lexer<'a> {
    pub fn new(sess: &Files, file_id: FileId) -> Lexer {
        Lexer {
            file_id,
            initial_len: sess.source(file_id).as_bytes().len(),
            chars: sess.source(file_id).chars(),
        }
    }

    fn index(&self) -> u32 {
        (self.initial_len - self.chars.as_str().as_bytes().len()) as u32
    }

    fn chars(&self) -> Chars<'a> {
        self.chars.clone()
    }

    fn nth_char(&mut self, n: usize) -> char {
        self.chars().nth(n).unwrap_or('\0')
    }

    fn skip_whitespace(&mut self) {
        loop {
            match self.nth_char(0) {
                c if c.is_whitespace() => self.chars.next(),
                _ => break,
            };
        }
    }

    fn next_literal(&mut self, first: char) -> String {
        let mut data = String::new();
        data.push(first);
        loop {
            match self.nth_char(0) {
                c if c.is_ascii_digit() => {
                    data.push(c);
                    self.chars.next();
                }
                _ => break data,
            }
        }
    }

    pub fn next_string_literal(&mut self) -> Option<Token> {
        let mut data = String::new();
        loop {
            match self.chars.next()? {
                '"' => break Some(Token::new(Kind::StringLiteral, &data)),
                c => {
                    data.push(c);
                }
            }
        }
    }

    pub fn next_ident_or_keyword(&mut self, first: char) -> Token {
        let mut data = String::new();
        data.push(first);
        loop {
            match self.nth_char(0) {
                c if c.is_alphanumeric() || c == '_' => {
                    data.push(c);
                    self.chars.next();
                }
                _ => break,
            }
        }
        match data.as_str() {
            "let" => Kind::Let.into(),
            "match" => Kind::Match.into(),
            "if" => Kind::If.into(),
            "else" => Kind::Else.into(),
            "func" => Kind::Func.into(),
            "struct" => Kind::Struct.into(),
            "enum" => Kind::Enum.into(),
            "i8" => Kind::I8Ty.into(),
            "i16" => Kind::I16Ty.into(),
            "i32" => Kind::I32Ty.into(),
            "i64" => Kind::I64Ty.into(),
            "string" => Kind::StringTy.into(),
            "env" if self.nth_char(0) == '!' => {
                self.chars.next();
                Kind::Env.into()
            }
            "println" => Kind::Println.into(),
            "socket" => Kind::Socket.into(),
            "bind" => Kind::Bind.into(),
            "listen" => Kind::Listen.into(),
            "accept" => Kind::Accept.into(),
            "recv" => Kind::Recv.into(),
            "send" => Kind::Send.into(),
            "close" => Kind::Close.into(),
            "listen_and_serve" => Kind::ListenAndServe.into(),
            data => Token::new(Kind::Ident, data),
        }
    }
}
