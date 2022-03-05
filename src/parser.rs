/// This is a recurisve descent parser for the P4 language.

use crate::lexer::{self, Lexer, Token, Kind};
use crate::error::{Error, ParserError};
use crate::ast::{
    AST, Type, Constant, Header, HeaderMember, Typedef, Control, Direction,
    ControlParameter, Action, Table, ActionParameter, MatchKind,
};

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

    fn parse_ref(&mut self) -> Result<String, Error> {
        let mut result = String::new();
        loop {
            let ident = self.parse_identifier()?;
            result = result + &ident;
            let token = self.next_token()?;
            match token.kind {
                lexer::Kind::Dot => result = result + ".",
                _ => {
                    self.backlog.push(token);
                    break;
                }
            } 
        }
        Ok(result)
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

            lexer::Kind::Identifier(name) => Type::UserDefined(name.clone()),

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

    pub fn handle_token(&mut self, token: Token, ast: &mut AST)
    -> Result<(), Error> {
        match token.kind {
            lexer::Kind::Const => self.handle_const_decl(ast)?,
            lexer::Kind::Header => self.handle_header_decl(ast)?,
            lexer::Kind::Typedef => self.handle_typedef(ast)?,
            lexer::Kind::Control => self.handle_control(ast)?,
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

    pub fn handle_header_decl(&mut self, ast: &mut AST)
    -> Result<(), Error> {

        // the first token of a header must be an identifier
        let name = self.parser.parse_identifier()?;

        // next the header body starts with an open curly brace
        self.parser.expect_token(lexer::Kind::CurlyOpen)?;

        let mut header = Header::new(name);

        // iterate over header members
        loop {
            let token = self.parser.next_token()?;

            // check if we've reached the end of the header body
            if token.kind == lexer::Kind::CurlyClose {
                break;
            }

            // if the token was not a closing curly bracket push it into the
            // backlog and carry on.
            self.parser.backlog.push(token);

            // parse a header member
            let ty = self.parser.parse_type()?;
            let name = self.parser.parse_identifier()?;
            self.parser.expect_token(lexer::Kind::Semicolon)?;

            header.members.push(HeaderMember{ty, name});
        }

        ast.headers.push(header);

        Ok(())
    }

    pub fn handle_typedef(&mut self, ast: &mut AST)
    -> Result<(), Error> {

        // first token must be a type
        let ty = self.parser.parse_type()?;

        // next must be a name
        let name = self.parser.parse_identifier()?;

        self.parser.expect_token(lexer::Kind::Semicolon)?;

        ast.typedefs.push(Typedef{ty, name});

        Ok(())

    }

    pub fn handle_control(&mut self, ast: &mut AST)
    -> Result<(), Error> {

        let mut cp = ControlParser::new(self.parser);
        let control = cp.run()?;
        ast.controls.push(control);
        Ok(())
    }

}

/// Parser for parsing control definitions
pub struct ControlParser<'a, 'b> {
    parser: &'b mut Parser<'a>,
}

impl<'a, 'b> ControlParser<'a, 'b> {

    pub fn new(parser: &'b mut Parser<'a>) -> Self {
        Self{ parser }
    }

    pub fn run(&mut self) -> Result<Control, Error> {

        let name = self.parser.parse_identifier()?;
        let mut control = Control::new(name);
        self.parse_parameters(&mut control)?;
        self.parse_body(&mut control)?;

        Ok(control)

    }

    pub fn parse_parameters(&mut self, control: &mut Control) -> Result<(), Error> {

        self.parser.expect_token(lexer::Kind::ParenOpen)?;

        loop {

            let token = self.parser.next_token()?;

            // check if we've reached the end of the parameters
            if token.kind == lexer::Kind::ParenClose {
                break;
            }
            
            // if the token was not a closing paren push it into the backlog and
            // carry on.
            self.parser.backlog.push(token);

            // parse a parameter
            let direction = self.parse_direction()?;
            let ty = self.parser.parse_type()?;
            let name = self.parser.parse_identifier()?;
            let token = self.parser.next_token()?;
            if token.kind == lexer::Kind::ParenClose {
                control.parameters.push(ControlParameter{direction, ty, name});
                break;
            }
            self.parser.backlog.push(token);
            self.parser.expect_token(lexer::Kind::Comma)?;

            control.parameters.push(ControlParameter{direction, ty, name});

        }

        Ok(())
    }

    pub fn parse_body(&mut self, control: &mut Control) -> Result<(), Error> {

        self.parser.expect_token(lexer::Kind::CurlyOpen)?;

        // iterate over body statements
        loop {
            let token = self.parser.next_token()?;

            match token.kind {
                lexer::Kind::CurlyClose => break,
                lexer::Kind::Action => self.parse_action(control)?,
                lexer::Kind::Table => self.parse_table(control)?,
                _ => {
                    return Err(ParserError{
                        at: token.clone(),
                        message: format!(
                            "Found {} expected: action, table or end of control",
                            token.kind,
                        ),
                        source: self.parser.lexer.lines[token.line].into()
                    }.into())
                }
            }

        }

        Ok(())

    }

    pub fn parse_action(&mut self, control: &mut Control) -> Result<(), Error> {

        let mut ap = ActionParser::new(self.parser);
        let action = ap.run()?;
        control.actions.push(action);

        Ok(())

    }

    pub fn parse_table(&mut self, control: &mut Control) -> Result<(), Error> {

        let mut tp = TableParser::new(self.parser);
        let table = tp.run()?;
        control.tables.push(table);

        Ok(())
    }

    pub fn parse_direction(&mut self) -> Result<Direction, Error> {

        let token = self.parser.next_token()?;
        match token.kind {
            lexer::Kind::In => Ok(Direction::In),
            lexer::Kind::Out => Ok(Direction::Out),
            lexer::Kind::InOut => Ok(Direction::InOut),
            _ => {
                Err(ParserError{
                    at: token.clone(),
                    message: format!(
                        "Found {} expected a direction: in, out or inout.",
                        token.kind,
                    ),
                    source: self.parser.lexer.lines[token.line].into(),
                }.into())
            }
        }

    }
}

pub struct ActionParser<'a, 'b> {
    parser: &'b mut Parser<'a>,
}

impl <'a, 'b> ActionParser<'a, 'b> {

    pub fn new(parser: &'b mut Parser<'a>) -> Self {
        Self { parser }
    }

    pub fn run(&mut self) -> Result<Action, Error> {

        let name = self.parser.parse_identifier()?;
        let mut action = Action::new(name);

        self.parse_parameters(&mut action)?;
        self.parse_body(&mut action)?;

        Ok(action)

    }

    pub fn parse_parameters(&mut self, action: &mut Action) -> Result<(), Error> {

        self.parser.expect_token(lexer::Kind::ParenOpen)?;

        loop {

            let token = self.parser.next_token()?;

            // check if we've reached the end of the parameters
            if token.kind == lexer::Kind::ParenClose {
                break;
            }
            
            // if the token was not a closing paren push it into the backlog and
            // carry on.
            self.parser.backlog.push(token);

            // parse a parameter
            let ty = self.parser.parse_type()?;
            let name = self.parser.parse_identifier()?;
            let token = self.parser.next_token()?;
            if token.kind == lexer::Kind::ParenClose {
                action.parameters.push(ActionParameter{ty, name});
                break;
            }
            self.parser.backlog.push(token);
            self.parser.expect_token(lexer::Kind::Comma)?;

            action.parameters.push(ActionParameter{ty, name});

        }

        Ok(())
    }

    pub fn parse_body(&mut self, _action: &mut Action) -> Result<(), Error> {
        self.parser.expect_token(lexer::Kind::CurlyOpen)?;

        loop {
            let token = self.parser.next_token()?;

            // check if we've reached the end of the parameters
            if token.kind == lexer::Kind::CurlyClose {
                break;
            }

            //TODO add body statements
        }

        Ok(())
    }

}

pub struct TableParser<'a, 'b> {
    parser: &'b mut Parser<'a>,
}

impl <'a, 'b> TableParser<'a, 'b> {

    pub fn new(parser: &'b mut Parser<'a>) -> Self {
        Self { parser }
    }

    pub fn run(&mut self) -> Result<Table, Error> {

        let name = self.parser.parse_identifier()?;
        let mut table = Table::new(name);

        self.parse_body(&mut table)?;
        
        Ok(table)

    }

    pub fn parse_body(&mut self, table: &mut Table) -> Result<(), Error> {

        self.parser.expect_token(lexer::Kind::CurlyOpen)?;

        loop {
            let token = self.parser.next_token()?;
            match token.kind {
                lexer::Kind::CurlyClose => break,
                lexer::Kind::Key => self.parse_key(table)?,
                lexer::Kind::Actions => self.parse_actions(table)?,
                lexer::Kind::DefaultAction => self.parse_default_action(table)?,
                lexer::Kind::Const => {
                    let token = self.parser.next_token()?;
                    match token.kind {
                        lexer::Kind::Entries => self.parse_entries(table)?,
                        //TODO need handle regular constants?
                        _ => {
                            return Err(ParserError{
                                at: token.clone(),
                                message: format!(
                                    "Found {} expected: entries",
                                    token.kind,
                                ),
                                source: self.parser.lexer.lines[token.line].into()
                            }.into())
                        }
                    }
                }
                _ => {
                    return Err(ParserError{
                        at: token.clone(),
                        message: format!(
                            "Found {} expected: key, actions, entries or end of \
                            table",
                            token.kind,
                        ),
                        source: self.parser.lexer.lines[token.line].into()
                    }.into())
                }
            }
        }

        Ok(())

    }

    pub fn parse_key(&mut self, table: &mut Table) -> Result<(), Error> {

        self.parser.expect_token(lexer::Kind::Equals)?;
        self.parser.expect_token(lexer::Kind::CurlyOpen)?;

        loop {
            let token = self.parser.next_token()?;

            // check if we've reached the end of the key block
            if token.kind == lexer::Kind::CurlyClose {
                break;
            }
            self.parser.backlog.push(token);

            let key = self.parser.parse_ref()?;
            self.parser.expect_token(lexer::Kind::Colon)?;
            let match_kind = self.parse_match_kind()?;
            self.parser.expect_token(lexer::Kind::Semicolon)?;

            table.key.insert(key, match_kind);

        }

        Ok(())

    }

    pub fn parse_match_kind(&mut self) -> Result<MatchKind, Error> {

        let token = self.parser.next_token()?;
        match token.kind {
            lexer::Kind::Exact => Ok(MatchKind::Exact),
            lexer::Kind::Ternary => Ok(MatchKind::Ternary),
            lexer::Kind::Lpm => Ok(MatchKind::LongestPrefixMatch),
            _ => {
                Err(ParserError{
                    at: token.clone(),
                    message: format!(
                        "Found {} expected match kind: exact, ternary or lpm",
                        token.kind,
                    ),
                    source: self.parser.lexer.lines[token.line].into(),
                }.into())
            }
        }
    }

    pub fn parse_actions(&mut self, table: &mut Table) -> Result<(), Error> {

        self.parser.expect_token(lexer::Kind::Equals)?;
        self.parser.expect_token(lexer::Kind::CurlyOpen)?;

        loop {
            let token = self.parser.next_token()?;

            // check if we've reached the end of the actions block
            if token.kind == lexer::Kind::CurlyClose {
                break;
            }
            self.parser.backlog.push(token);

            let action_name = self.parser.parse_identifier()?;
            self.parser.expect_token(lexer::Kind::Semicolon)?;

            table.actions.push(action_name);

        }

        Ok(())

    }

    pub fn parse_default_action(&mut self, table: &mut Table) -> Result<(), Error> {
        self.parser.expect_token(lexer::Kind::Equals)?;
        table.default_action = self.parser.parse_identifier()?;
        self.parser.expect_token(lexer::Kind::Semicolon)?;
        Ok(())
    }

    pub fn parse_entries(&mut self, _table: &mut Table) -> Result<(), Error> {

        self.parser.expect_token(lexer::Kind::Equals)?;
        self.parser.expect_token(lexer::Kind::CurlyOpen)?;

        loop {
            let token = self.parser.next_token()?;
            match token.kind {
                lexer::Kind::CurlyClose => break,
                _ => {
                    //TODO
                }
            }
        }

        Ok(())

    }

}
