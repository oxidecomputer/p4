use crate::lexer::Token;
use colored::Colorize;
use std::fmt;

#[derive(Debug)]
pub struct SemanticError {
    /// Token where the error was encountered
    pub at: Token,

    /// Message associated with this error.
    pub message: String,

    /// The source line the token error occured on.
    pub source: String,
}

impl fmt::Display for SemanticError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let loc = format!("\n[{}:{}]", self.at.line + 1, self.at.col + 1)
            .as_str()
            .bright_red();
        writeln!(f, "{} {}\n", loc, self.message.bright_white())?;
        writeln!(f, "  {}", self.source)?;

        // The presence of tabs makes presenting error indicators purely based
        // on column position impossible, so here we iterrate over the existing
        // string and mask out the non whitespace text inserting the error
        // indicators and preserving any tab/space mixture.
        let carat_line: String = self
            .source
            .chars()
            .enumerate()
            .map(|(i, x)| {
                if i == self.at.col {
                    return '^';
                }
                if x.is_whitespace() {
                    x
                } else {
                    ' '
                }
            })
            .collect();
        write!(f, "  {}", carat_line.bright_red())
    }
}

impl std::error::Error for SemanticError {}

#[derive(Debug)]
pub struct ParserError {
    /// Token where the error was encountered
    pub at: Token,

    /// Message associated with this error.
    pub message: String,

    /// The source line the token error occured on.
    pub source: String,
}

impl fmt::Display for ParserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let loc = format!("[{}:{}]", self.at.line + 1, self.at.col + 1)
            .as_str()
            .bright_red();
        writeln!(f, "{} {}\n", loc, self.message.bright_white())?;
        writeln!(f, "  {}", self.source)?;

        // The presence of tabs makes presenting error indicators purely based
        // on column position impossible, so here we iterrate over the existing
        // string and mask out the non whitespace text inserting the error
        // indicators and preserving any tab/space mixture.
        let carat_line: String = self
            .source
            .chars()
            .enumerate()
            .map(|(i, x)| {
                if i == self.at.col {
                    return '^';
                }
                if x.is_whitespace() {
                    x
                } else {
                    ' '
                }
            })
            .collect();
        write!(f, "  {}", carat_line.bright_red())
    }
}

impl std::error::Error for ParserError {}

#[derive(Debug)]
pub struct TokenError {
    /// Line where the token error was encountered.
    pub line: usize,

    /// Column where the token error was encountered.
    pub col: usize,

    /// Length of the erronious token.
    pub len: usize,

    /// The source line the token error occured on.
    pub source: String,
}

impl fmt::Display for TokenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let loc = format!("[{}:{}]", self.line + 1, self.col + 1)
            .as_str()
            .bright_red();
        writeln!(f, "{} {}\n", loc, "unrecognized token".bright_white())?;
        writeln!(f, "  {}", self.source)?;

        // The presence of tabs makes presenting error indicators purely based
        // on column position impossible, so here we iterrate over the existing
        // string and mask out the non whitespace text inserting the error
        // indicators and preserving any tab/space mixture.
        let carat_line: String = self
            .source
            .chars()
            .enumerate()
            .map(|(i, x)| {
                if i >= self.col && i < self.col + self.len {
                    return '^';
                }
                if x.is_whitespace() {
                    x
                } else {
                    ' '
                }
            })
            .collect();
        write!(f, "  {}", carat_line.bright_red())
    }
}

impl std::error::Error for TokenError {}

#[derive(Debug)]
pub enum Error {
    Lexer(TokenError),
    Parser(ParserError),
    Semantic(Vec<SemanticError>),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self {
            Self::Lexer(e) => e.fmt(f),
            Self::Parser(e) => e.fmt(f),
            Self::Semantic(errors) => {
                for e in errors {
                    e.fmt(f)?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for Error {}

impl From<TokenError> for Error {
    fn from(e: TokenError) -> Self {
        Self::Lexer(e)
    }
}

impl From<ParserError> for Error {
    fn from(e: ParserError) -> Self {
        Self::Parser(e)
    }
}

impl From<Vec<SemanticError>> for Error {
    fn from(e: Vec<SemanticError>) -> Self {
        Self::Semantic(e)
    }
}

#[derive(Debug)]
pub struct PreprocessorError {
    /// Token where the error was encountered
    pub line: usize,

    /// Message associated with this error.
    pub message: String,

    /// The source line the token error occured on.
    pub source: String,
}

impl fmt::Display for PreprocessorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let loc = format!("[{}]", self.line + 1).as_str().bright_red();
        writeln!(
            f,
            "{} {}: {}\n",
            loc,
            "preporcessor error".bright_white(),
            self.message.bright_white(),
        )?;
        writeln!(f, "  {}", self.source)
    }
}

impl std::error::Error for PreprocessorError {}
