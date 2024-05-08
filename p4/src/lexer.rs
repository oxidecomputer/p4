// Copyright 2022 Oxide Computer Company

use crate::error::TokenError;
use std::fmt;
use std::sync::Arc;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum Kind {
    //
    // keywords
    //
    Const,
    Header,
    Typedef,
    Control,
    Struct,
    Action,
    Parser,
    Table,
    Size,
    Key,
    Exact,
    Ternary,
    Lpm,
    Range,
    Actions,
    Counters,
    DefaultAction,
    Entries,
    In,
    InOut,
    Out,
    Transition,
    State,
    Select,
    Apply,
    Package,
    Extern,
    If,
    Else,
    Return,

    //
    // types
    //
    Bool,
    Error,
    Bit,
    Varbit,
    Int,
    String,

    //
    // lexical elements
    //
    AngleOpen,
    AngleClose,
    CurlyOpen,
    CurlyClose,
    ParenOpen,
    ParenClose,
    SquareOpen,
    SquareClose,
    Semicolon,
    Comma,
    Colon,
    Underscore,

    //
    // preprocessor
    //
    PoundInclude,
    PoundDefine,
    Backslash,
    Forwardslash,

    //
    // operators
    //
    DoubleEquals,
    NotEquals,
    Equals,
    Plus,
    Minus,
    Mod,
    Dot,
    Mask,
    LogicalAnd,
    And,
    Bang,
    Tilde,
    Shl,
    Pipe,
    Carat,
    GreaterThanEquals,
    LessThanEquals,

    //
    // literals
    //
    /// An integer literal. The following are literal examples and their
    /// associated types.
    ///     - `10`   : int
    ///     - `8s10` : int<8>
    ///     - `2s3`  : int<2>
    ///     - `1s1`  : int<1>
    IntLiteral(i128),

    Identifier(String),

    /// A bit literal. The following a literal examples and their associated
    /// types.
    ///     - `8w10` : bit<8>
    ///     - `1w1`  : bit<1>
    /// First element is number of bits (prefix before w) second element is
    /// value (suffix after w).
    BitLiteral(u16, u128),

    /// A signed literal. The following a literal examples and their associated
    /// types.
    ///     - `8s10` : bit<8>
    ///     - `1s1`  : bit<1>
    /// First element is number of bits (prefix before w) second element is
    /// value (suffix after w).
    SignedLiteral(u16, i128),

    TrueLiteral,
    FalseLiteral,
    StringLiteral(String),

    /// End of file.
    Eof,
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self {
            //
            // keywords
            //
            Kind::Const => write!(f, "keyword const"),
            Kind::Header => write!(f, "keyword header"),
            Kind::Typedef => write!(f, "keyword typedef"),
            Kind::Control => write!(f, "keyword control"),
            Kind::Struct => write!(f, "keyword struct"),
            Kind::Action => write!(f, "keyword action"),
            Kind::Parser => write!(f, "keyword parser"),
            Kind::Table => write!(f, "keyword table"),
            Kind::Size => write!(f, "keyword size"),
            Kind::Key => write!(f, "keyword key"),
            Kind::Exact => write!(f, "keyword exact"),
            Kind::Ternary => write!(f, "keyword ternary"),
            Kind::Lpm => write!(f, "keyword lpm"),
            Kind::Range => write!(f, "keyword range"),
            Kind::Actions => write!(f, "keyword actions"),
            Kind::Counters => write!(f, "keyword counters"),
            Kind::DefaultAction => write!(f, "keyword default_action"),
            Kind::Entries => write!(f, "keyword entries"),
            Kind::In => write!(f, "keyword in"),
            Kind::InOut => write!(f, "keyword in_out"),
            Kind::Out => write!(f, "keyword out"),
            Kind::Transition => write!(f, "keyword transition"),
            Kind::State => write!(f, "keyword state"),
            Kind::Select => write!(f, "keyword select"),
            Kind::Apply => write!(f, "keyword apply"),
            Kind::Package => write!(f, "keyword package"),
            Kind::Extern => write!(f, "keyword extern"),
            Kind::If => write!(f, "keyword if"),
            Kind::Else => write!(f, "keyword else"),
            Kind::Return => write!(f, "keyword return"),

            //
            // types
            //
            Kind::Bool => write!(f, "type bool"),
            Kind::Error => write!(f, "type error"),
            Kind::Bit => write!(f, "type bit"),
            Kind::Varbit => write!(f, "type varbit"),
            Kind::Int => write!(f, "type int"),
            Kind::String => write!(f, "type string"),

            //
            // lexical elements
            //
            Kind::AngleOpen => write!(f, "<"),
            Kind::AngleClose => write!(f, ">"),
            Kind::CurlyOpen => write!(f, "{{"),
            Kind::CurlyClose => write!(f, "}}"),
            Kind::ParenOpen => write!(f, "("),
            Kind::ParenClose => write!(f, ")"),
            Kind::SquareOpen => write!(f, "["),
            Kind::SquareClose => write!(f, "]"),
            Kind::Semicolon => write!(f, ";"),
            Kind::Comma => write!(f, ","),
            Kind::Colon => write!(f, ":"),
            Kind::Underscore => write!(f, "_"),

            //
            // preprocessor
            //
            Kind::PoundInclude => write!(f, "preprocessor statement #include"),
            Kind::PoundDefine => write!(f, "preprocessor statement #define"),
            Kind::Backslash => write!(f, "\\"),
            Kind::Forwardslash => write!(f, "/"),

            //
            // operators
            //
            Kind::DoubleEquals => write!(f, "operator =="),
            Kind::NotEquals => write!(f, "operator !="),
            Kind::Equals => write!(f, "operator ="),
            Kind::Plus => write!(f, "operator +"),
            Kind::Minus => write!(f, "operator -"),
            Kind::Mod => write!(f, "operator %"),
            Kind::Dot => write!(f, "operator ."),
            Kind::Mask => write!(f, "operator &&&"),
            Kind::LogicalAnd => write!(f, "operator &&"),
            Kind::And => write!(f, "operator &"),
            Kind::Bang => write!(f, "operator !"),
            Kind::Tilde => write!(f, "operator ~"),
            Kind::Shl => write!(f, "operator <<"),
            Kind::Pipe => write!(f, "operator |"),
            Kind::Carat => write!(f, "operator ^"),
            Kind::GreaterThanEquals => write!(f, "operator >="),
            Kind::LessThanEquals => write!(f, "operator <="),

            //
            // literals
            //
            Kind::IntLiteral(x) => write!(f, "int literal '{}'", x),
            Kind::Identifier(x) => write!(f, "identifier '{}'", x),
            Kind::BitLiteral(w, x) => write!(f, "bit literal '{}w{}'", w, x),
            Kind::SignedLiteral(w, x) => {
                write!(f, "signed literal {}s{}", w, x)
            }
            Kind::TrueLiteral => write!(f, "boolean literal true"),
            Kind::FalseLiteral => write!(f, "boolean literal false"),
            Kind::StringLiteral(x) => write!(f, "string literal '{}'", x),

            Kind::Eof => write!(f, "end of file"),
        }
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Token {
    /// The kind of token this is.
    pub kind: Kind,

    /// Line number of this token.
    pub line: usize,

    /// Column number of the first character in this token.
    pub col: usize,

    /// The file this token came from.
    pub file: Arc<String>,
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {:?}", self.line, self.col, self.kind)
    }
}

pub struct Lexer<'a> {
    pub line: usize,
    pub col: usize,
    pub show_tokens: bool,

    pub(crate) lines: Vec<&'a str>,
    cursor: &'a str,
    file: Arc<String>,
}

impl<'a> Lexer<'a> {
    pub fn new(lines: Vec<&'a str>, filename: Arc<String>) -> Self {
        if lines.is_empty() {
            return Self {
                cursor: "",
                line: 0,
                col: 0,
                lines,
                show_tokens: false,
                file: filename,
            };
        }

        let start = lines[0];

        Self {
            cursor: start,
            line: 0,
            col: 0,
            lines,
            show_tokens: false,
            file: filename,
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Result<Token, TokenError> {
        let token = self.do_next()?;
        if self.show_tokens {
            println!("{}", token);
        }
        Ok(token)
    }
    fn do_next(&mut self) -> Result<Token, TokenError> {
        self.check_end_of_line();

        if self.line >= self.lines.len() {
            return Ok(Token {
                kind: Kind::Eof,
                col: self.col,
                line: self.line,
                file: self.file.clone(),
            });
        }

        while self.skip_whitespace() {}
        while self.skip_comment() {}
        if self.line >= self.lines.len() {
            return Ok(Token {
                kind: Kind::Eof,
                col: self.col,
                line: self.line,
                file: self.file.clone(),
            });
        }
        self.skip_whitespace();
        //self.skip_comment();

        if let Some(t) = self.match_token("#include", Kind::PoundInclude) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("#define", Kind::PoundDefine) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("&&&", Kind::Mask) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("inout", Kind::InOut) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("in", Kind::In) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("out", Kind::Out) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("transition", Kind::Transition) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("state", Kind::State) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("select", Kind::Select) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("apply", Kind::Apply) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("package", Kind::Package) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("extern", Kind::Extern) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("if", Kind::If) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("else", Kind::Else) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("return", Kind::Return) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("&&", Kind::LogicalAnd) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("&", Kind::And) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("==", Kind::DoubleEquals) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("!=", Kind::NotEquals) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("|", Kind::Pipe) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("<<", Kind::Shl) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("<", Kind::AngleOpen) {
            return Ok(t);
        }

        if let Some(t) = self.match_token(">", Kind::AngleClose) {
            return Ok(t);
        }

        if let Some(t) = self.match_token(">=", Kind::GreaterThanEquals) {
            return Ok(t);
        }

        if let Some(t) = self.match_token(">=", Kind::LessThanEquals) {
            return Ok(t);
        }

        if let Some(t) = self.match_token(">", Kind::AngleClose) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("{", Kind::CurlyOpen) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("}", Kind::CurlyClose) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("(", Kind::ParenOpen) {
            return Ok(t);
        }

        if let Some(t) = self.match_token(")", Kind::ParenClose) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("[", Kind::SquareOpen) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("]", Kind::SquareClose) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("+", Kind::Plus) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("-", Kind::Minus) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("%", Kind::Mod) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("=", Kind::Equals) {
            return Ok(t);
        }

        if let Some(t) = self.match_token(":", Kind::Colon) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("_", Kind::Underscore) {
            return Ok(t);
        }

        if let Some(t) = self.match_token(";", Kind::Semicolon) {
            return Ok(t);
        }

        if let Some(t) = self.match_token(".", Kind::Dot) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("^", Kind::Carat) {
            return Ok(t);
        }

        if let Some(t) = self.match_token(",", Kind::Comma) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("!", Kind::Bang) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("~", Kind::Tilde) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("\\", Kind::Backslash) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("/", Kind::Forwardslash) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("bool", Kind::Bool) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("varbit", Kind::Varbit) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("bit", Kind::Bit) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("int", Kind::Int) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("typedef", Kind::Typedef) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("header", Kind::Header) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("const", Kind::Const) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("control", Kind::Control) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("struct", Kind::Struct) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("actions", Kind::Actions) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("counters", Kind::Counters) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("default_action", Kind::DefaultAction)
        {
            return Ok(t);
        }

        if let Some(t) = self.match_token("action", Kind::Action) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("parser", Kind::Parser) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("entries", Kind::Entries) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("table", Kind::Table) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("size", Kind::Size) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("key", Kind::Key) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("exact", Kind::Exact) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("ternary", Kind::Ternary) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("lpm", Kind::Lpm) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("range", Kind::Range) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("true", Kind::TrueLiteral) {
            return Ok(t);
        }

        if let Some(t) = self.match_token("false", Kind::FalseLiteral) {
            return Ok(t);
        }

        if let Some(t) = self.match_integer() {
            return Ok(t);
        }

        if let Some(t) = self.match_identifier() {
            return Ok(t);
        }

        let len = self.skip_token();

        Err(TokenError {
            line: self.line,
            col: self.col - len,
            source: self.lines[self.line].into(),
            file: self.file.clone(),
            len,
        })
    }

    fn match_identifier(&mut self) -> Option<Token> {
        let tok = self.peek_token();
        let len = tok.len();
        if tok.is_empty() {
            return None;
        }
        let mut chars = tok.chars();
        if !Self::is_letter(chars.next().unwrap()) {
            return None;
        }
        for c in chars {
            if !Self::is_letter(c) && !c.is_ascii_digit() {
                return None;
            }
        }
        let token = Token {
            kind: Kind::Identifier(tok.into()),
            col: self.col,
            line: self.line,
            file: self.file.clone(),
        };
        self.col += len;
        self.cursor = &self.cursor[len..];
        Some(token)
    }

    fn is_letter(c: char) -> bool {
        c.is_ascii_alphabetic() || c == '_'
    }

    fn parse_bitsized(
        &self,
        tok: &str,
        n: usize,
        ctor: fn(u16, u128) -> Kind,
    ) -> Option<Token> {
        let bits = match tok[..n].parse::<u16>() {
            Ok(n) => n,
            Err(_) => return None,
        };
        let value = if tok[n + 1..].starts_with("0x") {
            match u128::from_str_radix(&tok[n + 3..], 16) {
                Ok(n) => n,
                Err(_) => return None,
            }
        } else {
            match tok[n + 1..].parse::<u128>() {
                Ok(n) => n,
                Err(_) => return None,
            }
        };
        let token = Token {
            kind: ctor(bits, value),
            col: self.col,
            line: self.line,
            file: self.file.clone(),
        };
        Some(token)
    }

    // TODO copy pasta from parse_bitsized, but no trait to hold on to for
    // from_str_radix to generalize
    fn parse_intsized(
        &self,
        tok: &str,
        n: usize,
        ctor: fn(u16, i128) -> Kind,
    ) -> Option<Token> {
        let bits = match tok[..n].parse::<u16>() {
            Ok(n) => n,
            Err(_) => return None,
        };
        let value = if tok[n + 1..].starts_with("0x") {
            match i128::from_str_radix(&tok[n + 3..], 16) {
                Ok(n) => n,
                Err(_) => return None,
            }
        } else {
            match tok[n + 1..].parse::<i128>() {
                Ok(n) => n,
                Err(_) => return None,
            }
        };
        let token = Token {
            kind: ctor(bits, value),
            col: self.col,
            line: self.line,
            file: self.file.clone(),
        };
        Some(token)
    }

    fn match_integer(&mut self) -> Option<Token> {
        let tok = self.peek_token();
        let len = tok.len();
        if tok.is_empty() {
            return None;
        }

        let mut chars = tok.chars();
        let leading = if let Some('w') = chars.nth(1) {
            Some(1)
        } else if let Some('w') = chars.next() {
            Some(2)
        } else if let Some('w') = chars.next() {
            Some(3)
        } else {
            None
        };

        if let Some(n) = leading {
            match self.parse_bitsized(tok, n, Kind::BitLiteral) {
                Some(token) => {
                    self.col += len;
                    self.cursor = &self.cursor[len..];
                    return Some(token);
                }
                None => return None,
            }
        }

        let mut chars = tok.chars();
        let leading = if let Some('s') = chars.nth(1) {
            Some(1)
        } else if let Some('s') = chars.next() {
            Some(2)
        } else if let Some('s') = chars.next() {
            Some(3)
        } else {
            None
        };

        if let Some(n) = leading {
            match self.parse_intsized(tok, n, Kind::SignedLiteral) {
                Some(token) => {
                    self.col += len;
                    self.cursor = &self.cursor[len..];
                    return Some(token);
                }
                None => return None,
            }
        }

        let value = if let Some(tok) = tok.strip_prefix("0x") {
            let chars = tok.chars();
            for c in chars {
                if !c.is_ascii_hexdigit() {
                    return None;
                }
            }
            i128::from_str_radix(tok, 16).expect("parse hex int")
        } else {
            let chars = tok.chars();
            for c in chars {
                if !c.is_ascii_digit() {
                    return None;
                }
            }
            tok.parse::<i128>().expect("parse int")
        };
        let token = Token {
            kind: Kind::IntLiteral(value),
            col: self.col,
            line: self.line,
            file: self.file.clone(),
        };
        self.col += len;
        self.cursor = &self.cursor[len..];
        Some(token)
    }

    pub fn check_end_of_line(&mut self) -> bool {
        let mut end = false;
        while self.cursor.is_empty() {
            end = true;
            self.line += 1;
            self.col = 0;
            if self.line < self.lines.len() {
                self.cursor = self.lines[self.line];
            } else {
                break;
            }
        }
        end
    }

    fn skip_whitespace(&mut self) -> bool {
        let mut chars = self.cursor.chars();
        let mut skipped = false;
        while match chars.next() {
            Some(n) => n.is_whitespace(),
            None => false,
        } {
            skipped = true;
            self.cursor = &self.cursor[1..];
            self.col += 1;
            self.check_end_of_line();
        }
        skipped
    }

    fn skip_token(&mut self) -> usize {
        let mut len = 0;
        let mut chars = self.cursor.chars();
        while match chars.next() {
            Some(n) => !n.is_whitespace() && !Self::is_separator(n),
            None => false,
        } {
            len += 1
        }
        self.col += len;
        self.cursor = &self.cursor[len..];
        len
    }

    fn skip_comment(&mut self) -> bool {
        if self.cursor.starts_with("//") {
            self.skip_line_comment();
            return true;
        }
        if self.cursor.starts_with("/*") {
            self.skip_block_comment();
            return true;
        }
        false
    }

    fn skip_block_comment(&mut self) {
        let mut chars = self.cursor.chars();
        match chars.next() {
            Some('/') => {}
            _ => return,
        }
        match chars.next() {
            Some('*') => {}
            _ => return,
        }
        self.cursor = &self.cursor[2..];
        loop {
            loop {
                match chars.next() {
                    Some('*') => {
                        self.cursor = &self.cursor[1..];
                        match chars.next() {
                            Some('/') => {
                                self.col += 1;
                                self.cursor = &self.cursor[1..];
                                self.check_end_of_line();
                                self.skip_whitespace();
                                return;
                            }
                            _ => {
                                if self.check_end_of_line() {
                                    break;
                                }
                                self.cursor = &self.cursor[1..];
                                continue;
                            }
                        }
                    }
                    None => {
                        self.skip_whitespace();
                        if self.check_end_of_line() {
                            break;
                        }
                    }
                    _ => {
                        self.cursor = &self.cursor[1..];
                        continue;
                    }
                }
            }
            chars = self.cursor.chars();
        }
    }

    fn skip_line_comment(&mut self) {
        let mut chars = self.cursor.chars();
        match chars.next() {
            Some('/') => {}
            _ => return,
        }
        match chars.next() {
            Some('/') => {}
            _ => return,
        }
        let mut len = 2;
        while !matches!(chars.next(), Some('\r') | Some('\n') | None) {
            len += 1
        }
        self.col += len;
        self.cursor = &self.cursor[len..];
        self.check_end_of_line();
        self.skip_whitespace();
    }

    fn match_token(&mut self, text: &str, kind: Kind) -> Option<Token> {
        let tok = self.peek_token();
        let len = text.len();
        if tok.to_lowercase() == text.to_lowercase() {
            let token = Token {
                kind,
                col: self.col,
                line: self.line,
                file: self.file.clone(),
            };
            self.col += len;
            self.cursor = &self.cursor[len..];
            Some(token)
        } else {
            None
        }
    }

    fn peek_token(&self) -> &str {
        let mut chars = self.cursor.chars();

        // recognize non-space separators that should be tokens themselves
        match chars.next() {
            Some(';') => return &self.cursor[..1],
            Some(',') => return &self.cursor[..1],
            Some('+') => return &self.cursor[..1],
            Some('-') => return &self.cursor[..1],
            Some('(') => return &self.cursor[..1],
            Some(')') => return &self.cursor[..1],
            Some('{') => return &self.cursor[..1],
            Some('}') => return &self.cursor[..1],
            Some('[') => return &self.cursor[..1],
            Some(']') => return &self.cursor[..1],
            Some('.') => return &self.cursor[..1],
            Some(':') => return &self.cursor[..1],
            Some('*') => return &self.cursor[..1],
            Some('|') => return &self.cursor[..1],
            Some('~') => return &self.cursor[..1],
            Some('^') => return &self.cursor[..1],
            Some('\\') => return &self.cursor[..1],
            Some('/') => return &self.cursor[..1],
            Some('!') => match chars.next() {
                Some('=') => return &self.cursor[..2],
                _ => return &self.cursor[..1],
            },
            Some('=') => match chars.next() {
                Some('=') => return &self.cursor[..2],
                _ => return &self.cursor[..1],
            },
            Some('>') => match chars.next() {
                Some('=') => return &self.cursor[..2],
                _ => return &self.cursor[..1],
            },
            Some('<') => match chars.next() {
                Some('=') => return &self.cursor[..2],
                Some('<') => return &self.cursor[..2],
                _ => return &self.cursor[..1],
            },
            Some('&') => match chars.next() {
                Some('&') => match chars.next() {
                    Some('&') => return &self.cursor[..3],
                    _ => return &self.cursor[..2],
                },
                _ => return &self.cursor[..1],
            },
            _ => {}
        };

        let mut len = 1;
        while match chars.next() {
            Some(n) => !Self::is_separator(n),
            None => false,
        } {
            len += 1
        }
        &self.cursor[..len]
    }

    pub(crate) fn is_separator(c: char) -> bool {
        if c.is_whitespace() {
            return true;
        }
        if c == ',' {
            return true;
        }
        if c == ':' {
            return true;
        }
        if c == ';' {
            return true;
        }
        if c == '*' {
            return true;
        }
        if c == '+' {
            return true;
        }
        if c == '-' {
            return true;
        }
        if c == '<' {
            return true;
        }
        if c == '>' {
            return true;
        }
        if c == '{' {
            return true;
        }
        if c == '}' {
            return true;
        }
        if c == '=' {
            return true;
        }
        if c == '(' {
            return true;
        }
        if c == ')' {
            return true;
        }
        if c == '[' {
            return true;
        }
        if c == ']' {
            return true;
        }
        if c == '&' {
            return true;
        }
        if c == '.' {
            return true;
        }
        if c == '!' {
            return true;
        }
        if c == '^' {
            return true;
        }
        if c == '|' {
            return true;
        }
        if c == '~' {
            return true;
        }
        if c == '\\' {
            return true;
        }
        if c == '/' {
            return true;
        }
        false
    }
}
