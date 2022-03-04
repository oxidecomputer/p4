/// This is a recurisve descent parser for the P4 language.

use crate::lexer::{self, Lexer, Token, Kind};
use crate::error::{Error, ParserError};
use crate::ast::{AST, Type, Constant};

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    backlog: Vec<Token>,
}

impl<'a> Parser<'a> {
    pub fn new(lexer: Lexer<'a>) -> Self {
        Parser{ lexer, backlog: Vec::new() }
    }

    pub fn run(&mut self) -> Result<AST, Error> {

        let mut gp = GlobalParser::new(self);
        let ast = gp.run()?;
        Ok(ast)

    }

    pub fn next_token(&mut self) -> Result<Token, Error> {

        if self.backlog.is_empty() {
            Ok(self.lexer.next()?)
        } else {
            Ok(self.backlog.pop().unwrap())
        }

    }

    /// Consume a series of tokens constituting a path. Returns the first
    /// non-path element found.
    #[allow(dead_code)]
    fn parse_path(&mut self) -> Result<(String, Token), Error> {

        let mut path = String::new();
        loop {
            let token = self.next_token()?;
            match token.kind {
                lexer::Kind::Identifier(ref s) => path += s,
                lexer::Kind::Dot => path += ".",
                lexer::Kind::Forwardslash => path += "/",
                lexer::Kind::Backslash => path += "\\",
                _ => return Ok((path, token))
            }
        }
    }

    fn expect_token(
        &mut self,
        expected: lexer::Kind,
    )
    -> Result<(), Error> {

        let token = self.next_token()?;
        if token.kind != expected {
            return Err(ParserError{
                at: token.clone(),
                message: format!(
                    "Found {} expected '{}'.",
                    token.kind,
                    expected,
                ),
                source: self.lexer.lines[token.line].into(),
            }.into())
        }
        Ok(())

    }

    fn parse_identifier(&mut self) -> Result<String, Error> {

        let token = self.next_token()?;
        Ok(match token.kind {
            Kind::Identifier(name) => name,
            _ => {
                return Err(ParserError{
                    at: token.clone(),
                    message: format!(
                        "Found {} expected identifier.",
                        token.kind,
                    ),
                    source: self.lexer.lines[token.line].into(),
                }.into())
            }
        })

    }

    fn parse_type(&mut self) -> Result<Type, Error> {

        let token = self.next_token()?;
        Ok(match &token.kind {
            lexer::Kind::Bool => Type::Bool,
            lexer::Kind::Error => Type::Error,
            lexer::Kind::String => Type::String,
            lexer::Kind::Bit => 
                Type::Bit(self.parse_optional_width_parameter()?),

            lexer::Kind::Int =>
                Type::Int(self.parse_optional_width_parameter()?),

            _ => {
                return Err(ParserError{
                    at: token.clone(),
                    message: format!(
                        "Found {} expected type.",
                        token.kind,
                    ),
                    source: self.lexer.lines[token.line].into(),
                }.into())
            }
        })

    }

    fn parse_optional_width_parameter(
        &mut self
    ) -> Result<usize, Error> {

        let token = self.next_token()?;
        match &token.kind {
            lexer::Kind::AngleOpen => { }
            _ => {
                // no argument implies a size of 1 (7.1.6.2)
                self.backlog.push(token);
                return Ok(1);
            }
        }

        let token = self.next_token()?;
        let width = match &token.kind {
            lexer::Kind::IntLiteral(w) => w,
            _ => {
                return Err(ParserError{
                    at: token.clone(),
                    message: format!(
                        "Integer literal expected for width parameter, \
                        found {}",
                        token.kind,
                    ),
                    source: self.lexer.lines[token.line].into(),
                }.into())
            }
        };

        self.expect_token(Kind::AngleClose)?;


        Ok(*width as usize)

    }
}

/// Top level parser for parsing elements are global scope.
pub struct GlobalParser<'a, 'b> {
    parser: &'b mut Parser<'a>
}

impl<'a, 'b> GlobalParser<'a, 'b> {
    pub fn new(parser: &'b mut Parser<'a>) -> Self {
        Self{ parser }
    }

    pub fn run(&'b mut self) -> Result<AST, Error> {

        let mut prog = AST::default();

        loop {
            match self.parser.next_token() {
                Ok(token) => {
                    if token.kind == lexer::Kind::Eof {
                        break;
                    }
                    self.handle_token(token, &mut prog)?;
                }
                Err(e) => return Err(e)
            };
        }

        Ok(prog)

    }

    pub fn handle_token(&mut self, token: Token, prog: &mut AST)
    -> Result<(), Error> {
        match token.kind {
            lexer::Kind::Const => self.handle_const_decl(prog)?,
            _ => {}
        }
        Ok(())
    }

    pub fn handle_const_decl(&mut self, ast: &mut AST)
    -> Result<(), Error> {

        // the first token after const must be a type
        let ty = self.parser.parse_type()?;

        // next comes a name
        let name = self.parser.parse_identifier()?;

        ast.constants.push(Constant{ty, name});

        Ok(())
    }

}

