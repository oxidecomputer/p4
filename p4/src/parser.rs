use crate::ast::{
    self, Action, ActionParameter, ActionRef, BinOp, Call, ConstTableEntry,
    Constant, Control, ControlParameter, Direction, ElseIfBlock, Expression,
    ExpressionKind, Extern, ExternMethod, Header, HeaderMember, IfBlock,
    KeySetElement, KeySetElementValue, Lvalue, MatchKind, Package,
    PackageInstance, PackageParameter, Select, SelectElement, State, Statement,
    StatementBlock, Struct, StructMember, Table, Transition, Type, Typedef,
    Variable, AST,
};
use crate::error::{Error, ParserError};
/// This is a recurisve descent parser for the P4 language.
use crate::lexer::{self, Kind, Lexer, Token};

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    backlog: Vec<Token>,
}

impl<'a> Parser<'a> {
    pub fn new(lexer: Lexer<'a>) -> Self {
        Parser {
            lexer,
            backlog: Vec::new(),
        }
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
                _ => return Ok((path, token)),
            }
        }
    }

    fn expect_token(&mut self, expected: lexer::Kind) -> Result<(), Error> {
        let token = self.next_token()?;
        if token.kind != expected {
            return Err(ParserError {
                at: token.clone(),
                message: format!(
                    "Found {} expected '{}'.",
                    token.kind, expected,
                ),
                source: self.lexer.lines[token.line].into(),
            }
            .into());
        }
        Ok(())
    }

    fn parse_identifier(&mut self) -> Result<(String, Token), Error> {
        let token = self.next_token()?;
        Ok((
            match token.kind {
                Kind::Identifier(ref name) => name.into(),
                Kind::Apply => {
                    // sometimes apply is not the keyword but a method called
                    // against tables.
                    "apply".into()
                }
                _ => {
                    return Err(ParserError {
                        at: token.clone(),
                        message: format!(
                            "Found {} expected identifier.",
                            token.kind,
                        ),
                        source: self.lexer.lines[token.line].into(),
                    }
                    .into())
                }
            },
            token,
        ))
    }

    fn parse_lvalue(&mut self) -> Result<Lvalue, Error> {
        let mut name = String::new();
        let mut first_token = None;
        loop {
            let (ident, tk) = self.parse_identifier()?;
            match first_token {
                Some(_) => {}
                None => first_token = Some(tk),
            }
            name = name + &ident;
            let token = self.next_token()?;
            match token.kind {
                lexer::Kind::Dot => name += ".",
                _ => {
                    self.backlog.push(token);
                    break;
                }
            }
        }
        Ok(Lvalue {
            name,
            token: first_token.unwrap(),
        })
    }

    fn parse_type(&mut self) -> Result<(Type, Token), Error> {
        let token = self.next_token()?;
        Ok((
            match &token.kind {
                lexer::Kind::Bool => Type::Bool,
                lexer::Kind::Error => Type::Error,
                lexer::Kind::String => Type::String,
                lexer::Kind::Bit => {
                    Type::Bit(self.parse_optional_width_parameter()?)
                }

                lexer::Kind::Varbit => {
                    Type::Varbit(self.parse_optional_width_parameter()?)
                }

                lexer::Kind::Int => {
                    Type::Int(self.parse_optional_width_parameter()?)
                }

                lexer::Kind::Identifier(name) => {
                    Type::UserDefined(name.clone())
                }

                _ => {
                    return Err(
                        ParserError {
                            at: token.clone(),
                            message: format!(
                                "Found {} expected type.",
                                token.kind,
                            ),
                            source: self.lexer.lines[token.line].into(),
                        }
                        .into(),
                    )
                }
            },
            token,
        ))
    }

    fn parse_optional_width_parameter(&mut self) -> Result<usize, Error> {
        let token = self.next_token()?;
        match &token.kind {
            lexer::Kind::AngleOpen => {}
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
                return Err(ParserError {
                    at: token.clone(),
                    message: format!(
                        "Integer literal expected for width parameter, \
                        found {}",
                        token.kind,
                    ),
                    source: self.lexer.lines[token.line].into(),
                }
                .into())
            }
        };

        self.expect_token(Kind::AngleClose)?;

        Ok(*width as usize)
    }

    pub fn parse_direction(&mut self) -> Result<(Direction, Token), Error> {
        let token = self.next_token()?;
        match token.kind {
            lexer::Kind::In => Ok((Direction::In, token)),
            lexer::Kind::Out => Ok((Direction::Out, token)),
            lexer::Kind::InOut => Ok((Direction::InOut, token)),
            _ => {
                self.backlog.push(token.clone());
                Ok((Direction::Unspecified, token))
            }
        }
    }

    pub fn parse_variable(&mut self) -> Result<Variable, Error> {
        let (ty, _) = self.parse_type()?;
        let token = self.next_token()?;

        // check for constructor
        let parameters = if token.kind == lexer::Kind::ParenOpen {
            self.backlog.push(token);
            self.parse_parameters()?
        } else {
            self.backlog.push(token);
            Vec::new()
        };

        let (name, _) = self.parse_identifier()?;

        let token = self.next_token()?;
        // check for initializer
        if token.kind == lexer::Kind::Equals {
            let initializer = self.parse_expression()?;
            self.expect_token(lexer::Kind::Semicolon)?;
            Ok(Variable {
                ty,
                name,
                initializer: Some(initializer),
                parameters,
            })
        } else {
            self.backlog.push(token);
            self.expect_token(lexer::Kind::Semicolon)?;
            Ok(Variable {
                ty,
                name,
                initializer: None,
                parameters,
            })
        }
    }

    pub fn parse_constant(&mut self) -> Result<Constant, Error> {
        let (ty, _) = self.parse_type()?;
        let (name, _) = self.parse_identifier()?;
        self.expect_token(lexer::Kind::Equals)?;
        let initializer = self.parse_expression()?;
        self.expect_token(lexer::Kind::Semicolon)?;
        Ok(Constant {
            ty,
            name,
            initializer,
        })
    }

    pub fn parse_expression(&mut self) -> Result<Box<Expression>, Error> {
        let mut ep = ExpressionParser::new(self);
        ep.run()
    }

    pub fn parse_keyset(&mut self) -> Result<Vec<KeySetElement>, Error> {
        let token = self.next_token()?;
        match token.kind {
            lexer::Kind::ParenOpen => {
                // handle tuple set below
            }
            lexer::Kind::Underscore => {
                return Ok(vec![KeySetElement {
                    value: KeySetElementValue::DontCare,
                    token,
                }]);
            }
            _ => {
                self.backlog.push(token.clone());
                let mut ep = ExpressionParser::new(self);
                let expr = ep.run()?;
                return Ok(vec![KeySetElement {
                    value: KeySetElementValue::Expression(expr),
                    token,
                }]);
            }
        }

        let mut elements = Vec::new();

        loop {
            let token = self.next_token()?;
            // handle dont-care special case
            match token.kind {
                lexer::Kind::Underscore => {
                    elements.push(KeySetElement {
                        value: KeySetElementValue::DontCare,
                        token: token.clone(),
                    });
                    let token = self.next_token()?;
                    match token.kind {
                        lexer::Kind::Comma => continue,
                        lexer::Kind::ParenClose => return Ok(elements),
                        _ => {
                            return Err(ParserError {
                                at: token.clone(),
                                message: format!(
                                    "Found {} expected: \
                                    comma or paren close after \
                                    dont-care match",
                                    token.kind,
                                ),
                                source: self.lexer.lines[token.line].into(),
                            }
                            .into())
                        }
                    }
                }
                _ => {
                    self.backlog.push(token);
                }
            }
            let mut ep = ExpressionParser::new(self);
            let expr = ep.run()?;
            let token = self.next_token()?;
            match token.kind {
                lexer::Kind::Comma => {
                    elements.push(KeySetElement {
                        value: KeySetElementValue::Expression(expr),
                        token: token.clone(),
                    });
                    continue;
                }
                lexer::Kind::ParenClose => {
                    elements.push(KeySetElement {
                        value: KeySetElementValue::Expression(expr),
                        token,
                    });
                    return Ok(elements);
                }
                lexer::Kind::Mask => {
                    let mut ep = ExpressionParser::new(self);
                    let mask_expr = ep.run()?;
                    elements.push(KeySetElement {
                        value: KeySetElementValue::Masked(expr, mask_expr),
                        token: token.clone(),
                    });
                    let token = self.next_token()?;
                    match token.kind {
                        lexer::Kind::Comma => continue,
                        lexer::Kind::ParenClose => return Ok(elements),
                        _ => {
                            return Err(ParserError {
                                at: token.clone(),
                                message: format!(
                                    "Found {} expected: \
                                    comma or close paren after mask",
                                    token.kind,
                                ),
                                source: self.lexer.lines[token.line].into(),
                            }
                            .into())
                        }
                    }
                }
                //TODO Default case
                //TODO DontCare case
                _ => {
                    return Err(ParserError {
                        at: token.clone(),
                        message: format!(
                            "Found {} expected: keyset expression",
                            token.kind,
                        ),
                        source: self.lexer.lines[token.line].into(),
                    }
                    .into())
                }
            }
        }
    }

    // parse a tuple of expressions (<expr>, <expr> ...), used for both tuples
    // and function call sites
    pub fn parse_expr_parameters(
        &mut self,
    ) -> Result<Vec<Box<Expression>>, Error> {
        let mut result = Vec::new();

        self.expect_token(lexer::Kind::ParenOpen)?;

        loop {
            let token = self.next_token()?;

            // check if we've reached the end of the parameters
            if token.kind == lexer::Kind::ParenClose {
                break;
            }

            // if the token was not a closing paren push it into the backlog and
            // carry on.
            self.backlog.push(token);

            // parameters are a comma delimited list of expressions
            let mut ep = ExpressionParser::new(self);
            let expr = ep.run()?;
            result.push(expr);

            let token = self.next_token()?;
            match token.kind {
                lexer::Kind::Comma => continue,
                lexer::Kind::ParenClose => break,
                _ => {
                    // assume this token is a part of the next expression and
                    // carry on
                    self.backlog.push(token);
                    continue;
                }
            }
        }

        Ok(result)
    }

    fn try_parse_binop(&mut self) -> Result<Option<BinOp>, Error> {
        let token = self.next_token()?;
        match token.kind {
            lexer::Kind::GreaterThanEquals => Ok(Some(BinOp::Geq)),
            lexer::Kind::NotEquals => Ok(Some(BinOp::NotEq)),
            lexer::Kind::DoubleEquals => Ok(Some(BinOp::Eq)),
            lexer::Kind::Plus => Ok(Some(BinOp::Add)),
            lexer::Kind::Minus => Ok(Some(BinOp::Subtract)),
            lexer::Kind::Mask => Ok(Some(BinOp::Mask)),
            // TODO other binops
            _ => {
                self.backlog.push(token);
                Ok(None)
            }
        }
    }

    pub fn parse_statement_block(&mut self) -> Result<StatementBlock, Error> {
        let mut result = StatementBlock::default();

        self.expect_token(lexer::Kind::CurlyOpen)?;

        loop {
            let token = self.next_token()?;

            // check if we've reached the end of the statements
            match token.kind {
                lexer::Kind::CurlyClose => break,

                // variable declaration / initialization
                lexer::Kind::Bool
                | lexer::Kind::Error
                | lexer::Kind::Bit
                | lexer::Kind::Int
                | lexer::Kind::String => {
                    self.backlog.push(token);
                    let var = self.parse_variable()?;
                    result.statements.push(Statement::Variable(var));
                }

                // constant declaration / initialization
                lexer::Kind::Const => {
                    let c = self.parse_constant()?;
                    //result.constants.push(c);
                    result.statements.push(Statement::Constant(c));
                }

                lexer::Kind::Identifier(_)
                | lexer::Kind::If
                | lexer::Kind::Return => {
                    // push the identifier token into the backlog and run the
                    // statement parser
                    self.backlog.push(token);
                    let mut sp = StatementParser::new(self);
                    let stmt = sp.run()?;
                    result.statements.push(stmt);
                }
                lexer::Kind::Transition => {
                    result
                        .statements
                        .push(Statement::Transition(self.parse_transition()?));
                }

                _ => {
                    return Err(ParserError {
                        at: token.clone(),
                        message: format!(
                        "Found {} expected variable, constant, statement or \
                        instantiation.",
                        token.kind,
                    ),
                        source: self.lexer.lines[token.line].into(),
                    }
                    .into())
                }
            }
        }

        Ok(result)
    }

    pub fn parse_transition(&mut self) -> Result<Transition, Error> {
        let token = self.next_token()?;

        match token.kind {
            lexer::Kind::Select => {
                let mut sp = SelectParser::new(self);
                let select = sp.run()?;
                Ok(Transition::Select(select))
            }
            lexer::Kind::Identifier(name) => {
                let result = Transition::Reference(name);
                self.expect_token(lexer::Kind::Semicolon)?;
                Ok(result)
            }
            _ => Err(ParserError {
                at: token.clone(),
                message: format!(
                    "Found {}: expected select or identifier",
                    token.kind,
                ),
                source: self.lexer.lines[token.line].into(),
            }
            .into()),
        }
    }

    pub fn parse_parameters(&mut self) -> Result<Vec<ControlParameter>, Error> {
        let mut result = Vec::new();
        self.expect_token(lexer::Kind::ParenOpen)?;

        loop {
            let token = self.next_token()?;

            // check if we've reached the end of the parameters
            if token.kind == lexer::Kind::ParenClose {
                break;
            }

            // if the token was not a closing paren push it into the backlog and
            // carry on.
            self.backlog.push(token);

            // parse a parameter
            let (direction, dtk) = self.parse_direction()?;
            let (ty, ttk) = self.parse_type()?;
            let (name, ntk) = self.parse_identifier()?;
            let token = self.next_token()?;
            if token.kind == lexer::Kind::ParenClose {
                result.push(ControlParameter {
                    direction,
                    ty,
                    name,
                    dir_token: dtk,
                    ty_token: ttk,
                    name_token: ntk,
                });
                break;
            }
            self.backlog.push(token.clone());
            self.expect_token(lexer::Kind::Comma)?;

            result.push(ControlParameter {
                direction,
                ty,
                name,
                dir_token: dtk,
                ty_token: ttk,
                name_token: ntk,
            });
        }

        Ok(result)
    }

    pub fn parse_type_parameters(&mut self) -> Result<Vec<String>, Error> {
        let mut result = Vec::new();

        self.expect_token(lexer::Kind::AngleOpen)?;

        loop {
            let (ident, _) = self.parse_identifier()?;
            result.push(ident);

            let token = self.next_token()?;
            match token.kind {
                lexer::Kind::AngleClose => break,
                lexer::Kind::Comma => continue,
                _ => {
                    return Err(ParserError {
                        at: token.clone(),
                        message: format!(
                            "Found {} expected: type parameter",
                            token.kind,
                        ),
                        source: self.lexer.lines[token.line].into(),
                    }
                    .into())
                }
            }
        }

        Ok(result)
    }
}

/// Top level parser for parsing elements are global scope.
pub struct GlobalParser<'a, 'b> {
    parser: &'b mut Parser<'a>,
}

impl<'a, 'b> GlobalParser<'a, 'b> {
    pub fn new(parser: &'b mut Parser<'a>) -> Self {
        Self { parser }
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
                Err(e) => return Err(e),
            };
        }

        Ok(prog)
    }

    pub fn handle_token(
        &mut self,
        token: Token,
        ast: &mut AST,
    ) -> Result<(), Error> {
        match token.kind {
            lexer::Kind::Const => self.handle_const_decl(ast)?,
            lexer::Kind::Header => self.handle_header_decl(ast)?,
            lexer::Kind::Struct => self.handle_struct_decl(ast)?,
            lexer::Kind::Typedef => self.handle_typedef(ast)?,
            lexer::Kind::Control => self.handle_control(ast)?,
            lexer::Kind::Parser => self.handle_parser(ast, token)?,
            lexer::Kind::Package => self.handle_package(ast)?,
            lexer::Kind::Extern => self.handle_extern(ast)?,
            lexer::Kind::Identifier(typ) => {
                self.handle_package_instance(typ, ast)?
            }
            _ => {}
        }
        Ok(())
    }

    pub fn handle_const_decl(&mut self, ast: &mut AST) -> Result<(), Error> {
        // the first token after const must be a type
        let (ty, _) = self.parser.parse_type()?;

        // next comes a name
        let (name, _) = self.parser.parse_identifier()?;

        // then an initializer
        self.parser.expect_token(lexer::Kind::Equals)?;
        let initializer = self.parser.parse_expression()?;

        ast.constants.push(Constant {
            ty,
            name,
            initializer,
        });

        Ok(())
    }

    pub fn handle_header_decl(&mut self, ast: &mut AST) -> Result<(), Error> {
        // the first token of a header must be an identifier
        let (name, _) = self.parser.parse_identifier()?;

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
            let (ty, tyt) = self.parser.parse_type()?;
            let (name, _) = self.parser.parse_identifier()?;
            self.parser.expect_token(lexer::Kind::Semicolon)?;

            header.members.push(HeaderMember { ty, name, token: tyt });
        }

        ast.headers.push(header);

        Ok(())
    }

    pub fn handle_struct_decl(&mut self, ast: &mut AST) -> Result<(), Error> {
        // the first token of a struct must be an identifier
        let (name, _) = self.parser.parse_identifier()?;

        // next the struct body starts with an open curly brace
        self.parser.expect_token(lexer::Kind::CurlyOpen)?;

        let mut p4_struct = Struct::new(name);

        // iterate over struct members
        loop {
            let token = self.parser.next_token()?;

            // check if we've reached the end of the struct body
            if token.kind == lexer::Kind::CurlyClose {
                break;
            }

            // if the token was not a closing curly bracket push it into the
            // backlog and carry on.
            self.parser.backlog.push(token);

            // parse a struct member
            let (ty, tyt) = self.parser.parse_type()?;
            let (name, _) = self.parser.parse_identifier()?;
            self.parser.expect_token(lexer::Kind::Semicolon)?;

            p4_struct.members.push(StructMember {
                ty,
                name,
                token: tyt,
            });
        }

        ast.structs.push(p4_struct);

        Ok(())
    }

    pub fn handle_typedef(&mut self, ast: &mut AST) -> Result<(), Error> {
        // first token must be a type
        let (ty, _) = self.parser.parse_type()?;

        // next must be a name
        let (name, _) = self.parser.parse_identifier()?;

        self.parser.expect_token(lexer::Kind::Semicolon)?;

        ast.typedefs.push(Typedef { ty, name });

        Ok(())
    }

    pub fn handle_control(&mut self, ast: &mut AST) -> Result<(), Error> {
        let mut cp = ControlParser::new(self.parser);
        let control = cp.run()?;
        ast.controls.push(control);
        Ok(())
    }

    pub fn handle_parser(
        &mut self,
        ast: &mut AST,
        start: Token,
    ) -> Result<(), Error> {
        let mut pp = ParserParser::new(self.parser, start);
        let parser = pp.run()?;
        ast.parsers.push(parser);
        Ok(())
    }

    pub fn handle_package(&mut self, ast: &mut AST) -> Result<(), Error> {
        let (name, _) = self.parser.parse_identifier()?;
        let mut pkg = Package::new(name);

        let token = self.parser.next_token()?;
        match token.kind {
            lexer::Kind::AngleOpen => {
                self.parser.backlog.push(token);
                pkg.type_parameters = self.parser.parse_type_parameters()?;
            }
            _ => {
                self.parser.backlog.push(token);
            }
        }

        self.parse_package_parameters(&mut pkg)?;
        self.parser.expect_token(lexer::Kind::Semicolon)?;

        ast.packages.push(pkg);

        Ok(())
    }

    pub fn handle_extern(&mut self, ast: &mut AST) -> Result<(), Error> {
        let (name, token) = self.parser.parse_identifier()?;
        self.parser.expect_token(lexer::Kind::CurlyOpen)?;

        let mut ext = Extern {
            name,
            token,
            methods: Vec::new(),
        };

        // parse functions
        loop {
            let (return_type, _) = self.parser.parse_type()?;
            let (name, _) = self.parser.parse_identifier()?;

            let token = self.parser.next_token()?;
            let type_parameters = if token.kind == lexer::Kind::AngleOpen {
                self.parser.backlog.push(token);
                self.parser.parse_type_parameters()?
            } else {
                self.parser.backlog.push(token);
                Vec::new()
            };
            let parameters = self.parser.parse_parameters()?;
            self.parser.expect_token(lexer::Kind::Semicolon)?;

            ext.methods.push(ExternMethod {
                return_type,
                name,
                type_parameters,
                parameters,
            });

            let token = self.parser.next_token()?;
            if token.kind == lexer::Kind::CurlyClose {
                break;
            } else {
                self.parser.backlog.push(token);
            }
        }

        ast.externs.push(ext);

        Ok(())
    }

    pub fn parse_package_parameters(
        &mut self,
        pkg: &mut Package,
    ) -> Result<(), Error> {
        self.parser.expect_token(lexer::Kind::ParenOpen)?;
        loop {
            let token = self.parser.next_token()?;
            match token.kind {
                lexer::Kind::ParenClose => break,
                lexer::Kind::Comma => continue,
                lexer::Kind::Identifier(type_name) => {
                    let mut parameter = PackageParameter::new(type_name);
                    let token = self.parser.next_token()?;
                    self.parser.backlog.push(token.clone());
                    if token.kind == lexer::Kind::AngleOpen {
                        parameter.type_parameters =
                            self.parser.parse_type_parameters()?;
                    }
                    let (name, _) = self.parser.parse_identifier()?;
                    parameter.name = name;
                    pkg.parameters.push(parameter);
                }
                _ => {
                    return Err(ParserError {
                        at: token.clone(),
                        message: format!(
                            "Found {} expected package parameter.",
                            token.kind,
                        ),
                        source: self.parser.lexer.lines[token.line].into(),
                    }
                    .into())
                }
            }
        }

        Ok(())
    }

    pub fn handle_package_instance(
        &mut self,
        typ: String,
        ast: &mut AST,
    ) -> Result<(), Error> {
        let mut inst = PackageInstance::new(typ);

        self.parser.expect_token(lexer::Kind::ParenOpen)?;
        loop {
            let (arg, _) = self.parser.parse_identifier()?;
            self.parser.expect_token(lexer::Kind::ParenOpen)?;
            self.parser.expect_token(lexer::Kind::ParenClose)?;
            inst.parameters.push(arg);
            let token = self.parser.next_token()?;
            match token.kind {
                lexer::Kind::ParenClose => break,
                _ => {
                    self.parser.backlog.push(token);
                    self.parser.expect_token(lexer::Kind::Comma)?;
                    continue;
                }
            }
        }

        let (name, _) = self.parser.parse_identifier()?;
        inst.name = name;
        self.parser.expect_token(lexer::Kind::Semicolon)?;

        ast.package_instance = Some(inst);
        Ok(())
    }
}

/// Parser for parsing control definitions
pub struct ControlParser<'a, 'b> {
    parser: &'b mut Parser<'a>,
}

impl<'a, 'b> ControlParser<'a, 'b> {
    pub fn new(parser: &'b mut Parser<'a>) -> Self {
        Self { parser }
    }

    pub fn run(&mut self) -> Result<Control, Error> {
        let (name, _) = self.parser.parse_identifier()?;
        let mut control = Control::new(name);

        //
        // check for type parameters
        //

        let token = self.parser.next_token()?;
        match token.kind {
            lexer::Kind::AngleOpen => {
                self.parser.backlog.push(token);
                control.type_parameters =
                    self.parser.parse_type_parameters()?;
            }
            _ => {
                self.parser.backlog.push(token);
            }
        }

        //
        // control parameters
        //

        control.parameters = self.parser.parse_parameters()?;

        //
        // check for declaration only (e.g. no body)
        //

        let token = self.parser.next_token()?;
        match token.kind {
            lexer::Kind::Semicolon => return Ok(control),
            _ => {
                self.parser.backlog.push(token);
            }
        }

        //
        // parse body of the control
        //

        self.parse_body(&mut control)?;

        Ok(control)
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
                lexer::Kind::Apply => self.parse_apply(control)?,
                lexer::Kind::Const => {
                    let c = self.parser.parse_constant()?;
                    control.constants.push(c);
                }
                lexer::Kind::Identifier(_) => {
                    self.parser.backlog.push(token);
                    let v = self.parser.parse_variable()?;
                    control.variables.push(v);
                }
                _ => {
                    return Err(ParserError {
                        at: token.clone(),
                        message: format!(
                            "Found {} expected: \
                            action, table or end of control",
                            token.kind,
                        ),
                        source: self.parser.lexer.lines[token.line].into(),
                    }
                    .into())
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

    pub fn parse_apply(&mut self, control: &mut Control) -> Result<(), Error> {
        control.apply = self.parser.parse_statement_block()?;

        Ok(())
    }
}

pub struct ActionParser<'a, 'b> {
    parser: &'b mut Parser<'a>,
}

impl<'a, 'b> ActionParser<'a, 'b> {
    pub fn new(parser: &'b mut Parser<'a>) -> Self {
        Self { parser }
    }

    pub fn run(&mut self) -> Result<Action, Error> {
        let (name, _) = self.parser.parse_identifier()?;
        let mut action = Action::new(name);

        self.parse_parameters(&mut action)?;
        //self.parse_body(&mut action)?;
        action.statement_block = self.parser.parse_statement_block()?;

        Ok(action)
    }

    pub fn parse_parameters(
        &mut self,
        action: &mut Action,
    ) -> Result<(), Error> {
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
            let direction = match self.parser.parse_direction() {
                Ok((dir, _)) => dir,
                Err(_) => Direction::Unspecified,
            };
            let (ty, ty_token) = self.parser.parse_type()?;
            let (name, name_token) = self.parser.parse_identifier()?;
            let token = self.parser.next_token()?;
            if token.kind == lexer::Kind::ParenClose {
                action.parameters.push(ActionParameter {
                    direction,
                    ty,
                    name,
                    ty_token,
                    name_token,
                });
                break;
            }
            self.parser.backlog.push(token);
            self.parser.expect_token(lexer::Kind::Comma)?;

            action.parameters.push(ActionParameter {
                direction,
                ty,
                name,
                ty_token,
                name_token,
            });
        }

        Ok(())
    }

    pub fn parse_sized_variable(
        &mut self,
        _ty: Type,
    ) -> Result<Variable, Error> {
        todo!();
    }
}

pub struct TableParser<'a, 'b> {
    parser: &'b mut Parser<'a>,
}

impl<'a, 'b> TableParser<'a, 'b> {
    pub fn new(parser: &'b mut Parser<'a>) -> Self {
        Self { parser }
    }

    pub fn run(&mut self) -> Result<Table, Error> {
        let (name, _) = self.parser.parse_identifier()?;
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
                lexer::Kind::DefaultAction => {
                    self.parse_default_action(table)?
                }
                lexer::Kind::Size => {
                    self.parser.expect_token(lexer::Kind::Equals)?;
                    let token = self.parser.next_token()?;
                    let size = match token.kind {
                        lexer::Kind::IntLiteral(x) => x,
                        _ => {
                            return Err(ParserError {
                                at: token.clone(),
                                message: format!(
                                    "Found {} expected constant integer",
                                    token.kind,
                                ),
                                source: self.parser.lexer.lines[token.line]
                                    .into(),
                            }
                            .into())
                        }
                    };
                    self.parser.expect_token(lexer::Kind::Semicolon)?;
                    table.size = size as usize;
                }
                lexer::Kind::Const => {
                    let token = self.parser.next_token()?;
                    match token.kind {
                        lexer::Kind::Entries => self.parse_entries(table)?,
                        //TODO need handle regular constants?
                        _ => {
                            return Err(ParserError {
                                at: token.clone(),
                                message: format!(
                                    "Found {} expected: entries",
                                    token.kind,
                                ),
                                source: self.parser.lexer.lines[token.line]
                                    .into(),
                            }
                            .into())
                        }
                    }
                }
                _ => {
                    return Err(ParserError {
                        at: token.clone(),
                        message: format!(
                        "Found {} expected: key, actions, entries or end of \
                            table",
                        token.kind,
                    ),
                        source: self.parser.lexer.lines[token.line].into(),
                    }
                    .into())
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

            let key = self.parser.parse_lvalue()?;
            self.parser.expect_token(lexer::Kind::Colon)?;
            let match_kind = self.parse_match_kind()?;
            self.parser.expect_token(lexer::Kind::Semicolon)?;

            table.key.push((key, match_kind));
        }

        Ok(())
    }

    pub fn parse_match_kind(&mut self) -> Result<MatchKind, Error> {
        let token = self.parser.next_token()?;
        match token.kind {
            lexer::Kind::Exact => Ok(MatchKind::Exact),
            lexer::Kind::Ternary => Ok(MatchKind::Ternary),
            lexer::Kind::Lpm => Ok(MatchKind::LongestPrefixMatch),
            _ => Err(ParserError {
                at: token.clone(),
                message: format!(
                    "Found {} expected match kind: exact, ternary or lpm",
                    token.kind,
                ),
                source: self.parser.lexer.lines[token.line].into(),
            }
            .into()),
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

            let (action_name, _) = self.parser.parse_identifier()?;
            self.parser.expect_token(lexer::Kind::Semicolon)?;

            table.actions.push(action_name);
        }

        Ok(())
    }

    pub fn parse_default_action(
        &mut self,
        table: &mut Table,
    ) -> Result<(), Error> {
        self.parser.expect_token(lexer::Kind::Equals)?;
        (table.default_action, _) = self.parser.parse_identifier()?;
        self.parser.expect_token(lexer::Kind::Semicolon)?;
        Ok(())
    }

    pub fn parse_entries(&mut self, table: &mut Table) -> Result<(), Error> {
        self.parser.expect_token(lexer::Kind::Equals)?;
        self.parser.expect_token(lexer::Kind::CurlyOpen)?;

        loop {
            let token = self.parser.next_token()?;
            match token.kind {
                lexer::Kind::CurlyClose => break,
                _ => {
                    self.parser.backlog.push(token);
                    let entry = self.parse_entry()?;
                    table.const_entries.push(entry);
                }
            }
        }

        Ok(())
    }

    pub fn parse_entry(&mut self) -> Result<ConstTableEntry, Error> {
        let keyset = self.parser.parse_keyset()?;
        self.parser.expect_token(lexer::Kind::Colon)?;
        let action = self.parse_actionref()?;
        self.parser.expect_token(lexer::Kind::Semicolon)?;
        Ok(ConstTableEntry { keyset, action })
    }

    pub fn parse_actionref(&mut self) -> Result<ActionRef, Error> {
        let (name, aref_tk) = self.parser.parse_identifier()?;
        let token = self.parser.next_token()?;
        let mut actionref = ActionRef::new(name, aref_tk);
        match token.kind {
            lexer::Kind::Semicolon => Ok(actionref),
            lexer::Kind::ParenOpen => {
                let token = self.parser.next_token()?;
                if token.kind == lexer::Kind::ParenClose {
                    return Ok(actionref);
                } else {
                    self.parser.backlog.push(token);
                }
                let mut args = Vec::new();
                loop {
                    let mut ep = ExpressionParser::new(self.parser);
                    let expr = ep.run()?;
                    let token = self.parser.next_token()?;
                    match token.kind {
                        lexer::Kind::Comma => {
                            args.push(expr);
                            continue;
                        }
                        lexer::Kind::ParenClose => {
                            args.push(expr);
                            actionref.parameters = args;
                            return Ok(actionref);
                        }
                        _ => {
                            return Err(ParserError {
                                at: token.clone(),
                                message: format!(
                                    "Found {} expected: action parameter",
                                    token.kind,
                                ),
                                source: self.parser.lexer.lines[token.line]
                                    .into(),
                            }
                            .into())
                        }
                    }
                }
            }
            _ => Err(ParserError {
                at: token.clone(),
                message: format!(
                    "Found {} expected: reference to action, or \
                        parameterized reference to action",
                    token.kind,
                ),
                source: self.parser.lexer.lines[token.line].into(),
            }
            .into()),
        }
    }
}

pub struct StatementParser<'a, 'b> {
    parser: &'b mut Parser<'a>,
}

impl<'a, 'b> StatementParser<'a, 'b> {
    pub fn new(parser: &'b mut Parser<'a>) -> Self {
        Self { parser }
    }

    pub fn run(&mut self) -> Result<Statement, Error> {
        let token = self.parser.next_token()?;
        match token.kind {
            lexer::Kind::If => {
                let mut iep = IfElseParser::new(self.parser);
                return iep.run();
            }
            lexer::Kind::Return => {
                let token = self.parser.next_token()?;
                if token.kind == lexer::Kind::Semicolon {
                    return Ok(Statement::Return(None));
                } else {
                    self.parser.backlog.push(token);
                    let mut ep = ExpressionParser::new(self.parser);
                    return Ok(Statement::Return(Some(ep.run()?)));
                }
            }
            _ => {
                self.parser.backlog.push(token);
            }
        }

        // wrap the identifier as an lvalue, consuming any dot
        // concatenated references
        let lval = self.parser.parse_lvalue()?;

        let token = self.parser.next_token()?;
        let statement = match token.kind {
            lexer::Kind::Equals => self.parse_assignment(lval)?,
            lexer::Kind::ParenOpen => {
                self.parser.backlog.push(token);
                self.parse_call(lval)?
            }
            lexer::Kind::AngleOpen => self.parse_parameterized_call(lval)?,
            _ => {
                return Err(ParserError {
                    at: token.clone(),
                    message: format!(
                        "Found {} expected assignment or function/method call.",
                        token.kind,
                    ),
                    source: self.parser.lexer.lines[token.line].into(),
                }
                .into())
            }
        };

        self.parser.expect_token(lexer::Kind::Semicolon)?;
        Ok(statement)
    }

    pub fn parse_assignment(
        &mut self,
        lval: Lvalue,
    ) -> Result<Statement, Error> {
        let mut ep = ExpressionParser::new(self.parser);
        let expression = ep.run()?;
        Ok(Statement::Assignment(lval, expression))
    }

    pub fn parse_call(&mut self, lval: Lvalue) -> Result<Statement, Error> {
        let args = self.parser.parse_expr_parameters()?;
        Ok(Statement::Call(Call { lval, args }))
    }

    pub fn parse_parameterized_call(
        &mut self,
        _lval: Lvalue,
    ) -> Result<Statement, Error> {
        todo!();
    }
}

pub struct IfElseParser<'a, 'b> {
    parser: &'b mut Parser<'a>,
}

impl<'a, 'b> IfElseParser<'a, 'b> {
    pub fn new(parser: &'b mut Parser<'a>) -> Self {
        Self { parser }
    }

    pub fn run(&mut self) -> Result<Statement, Error> {
        let predicate = self.parse_predicate()?;
        let block = self.parser.parse_statement_block()?;
        let mut blk = IfBlock {
            predicate,
            block,
            else_ifs: Vec::new(),
            else_block: None,
        };

        // check for else / else if
        loop {
            let token = self.parser.next_token()?;
            if token.kind == lexer::Kind::Else {
                let token = self.parser.next_token()?;
                if token.kind == lexer::Kind::If {
                    // else if
                    let predicate = self.parse_predicate()?;
                    let block = self.parser.parse_statement_block()?;
                    blk.else_ifs.push(ElseIfBlock { predicate, block });
                } else {
                    // else
                    self.parser.backlog.push(token);
                    let block = self.parser.parse_statement_block()?;
                    blk.else_block = Some(block);
                    break;
                }
            } else {
                self.parser.backlog.push(token);
                break;
            }
        }

        Ok(Statement::If(blk))
    }

    fn parse_predicate(&mut self) -> Result<Box<Expression>, Error> {
        self.parser.expect_token(lexer::Kind::ParenOpen)?;
        let mut ep = ExpressionParser::new(self.parser);
        let xpr = ep.run()?;
        self.parser.expect_token(lexer::Kind::ParenClose)?;
        Ok(xpr)
    }
}

pub struct ExpressionParser<'a, 'b> {
    parser: &'b mut Parser<'a>,
}

impl<'a, 'b> ExpressionParser<'a, 'b> {
    pub fn new(parser: &'b mut Parser<'a>) -> Self {
        Self { parser }
    }

    pub fn run(&mut self) -> Result<Box<Expression>, Error> {
        let token = self.parser.next_token()?;
        let lhs = match token.kind {
            lexer::Kind::TrueLiteral => {
                Expression::new(token.clone(), ExpressionKind::BoolLit(true))
            }
            lexer::Kind::FalseLiteral => {
                Expression::new(token.clone(), ExpressionKind::BoolLit(false))
            }
            lexer::Kind::IntLiteral(value) => Expression::new(
                token.clone(),
                ExpressionKind::IntegerLit(value),
            ),
            lexer::Kind::BitLiteral(width, value) => Expression::new(
                token.clone(),
                ExpressionKind::BitLit(width, value),
            ),
            lexer::Kind::SignedLiteral(width, value) => Expression::new(
                token.clone(),
                ExpressionKind::SignedLit(width, value),
            ),
            lexer::Kind::Identifier(_) => {
                self.parser.backlog.push(token.clone());
                let lval = self.parser.parse_lvalue()?;

                let token = self.parser.next_token()?;

                // check for index
                if token.kind == lexer::Kind::SquareOpen {
                    let mut xp = ExpressionParser::new(self.parser);
                    let xpr = xp.run()?;
                    // check for slice
                    let slice_token = self.parser.next_token()?;
                    if slice_token.kind == lexer::Kind::Colon {
                        let mut xp = ExpressionParser::new(self.parser);
                        let slice_xpr = xp.run()?;
                        self.parser.expect_token(lexer::Kind::SquareClose)?;
                        Expression::new(
                            token,
                            ExpressionKind::Index(
                                lval,
                                Expression::new(
                                    slice_token,
                                    ExpressionKind::Slice(xpr, slice_xpr),
                                ),
                            ),
                        )
                    } else {
                        self.parser.backlog.push(token.clone());
                        self.parser.expect_token(lexer::Kind::SquareClose)?;
                        Expression::new(token, ExpressionKind::Index(lval, xpr))
                    }
                }
                // check for call
                else if token.kind == lexer::Kind::ParenOpen {
                    self.parser.backlog.push(token.clone());
                    let args = self.parser.parse_expr_parameters()?;
                    Expression::new(
                        token,
                        ExpressionKind::Call(Call { lval, args }),
                    )
                }
                // if it's not an index and it's not a call, it's an lvalue
                else {
                    self.parser.backlog.push(token.clone());
                    Expression::new(token, ExpressionKind::Lvalue(lval))
                }
            }
            _ => {
                return Err(ParserError {
                    at: token.clone(),
                    message: format!(
                        "Found {} expected expression.",
                        token.kind,
                    ),
                    source: self.parser.lexer.lines[token.line].into(),
                }
                .into())
            }
        };

        // check for binary operator
        match self.parser.try_parse_binop()? {
            Some(op) => {
                // recurse to rhs
                let mut ep = ExpressionParser::new(self.parser);
                let rhs = ep.run()?;
                Ok(Expression::new(token, ExpressionKind::Binary(lhs, op, rhs)))
            }
            None => Ok(lhs),
        }
    }
}

/// Parser for parsing parser definitions
pub struct ParserParser<'a, 'b> {
    parser: &'b mut Parser<'a>,
    start: Token,
}

impl<'a, 'b> ParserParser<'a, 'b> {
    pub fn new(parser: &'b mut Parser<'a>, start: Token) -> Self {
        Self { parser, start }
    }

    pub fn run(&mut self) -> Result<ast::Parser, Error> {
        let (name, _) = self.parser.parse_identifier()?;
        let mut parser = ast::Parser::new(name, self.start.clone());

        let token = self.parser.next_token()?;
        match token.kind {
            lexer::Kind::AngleOpen => {
                self.parser.backlog.push(token);
                parser.type_parameters = self.parser.parse_type_parameters()?;
            }
            _ => {
                self.parser.backlog.push(token);
            }
        }

        self.parse_parameters(&mut parser)?;

        let token = self.parser.next_token()?;
        match token.kind {
            lexer::Kind::Semicolon => {
                parser.decl_only = true;
                return Ok(parser);
            }
            _ => {
                self.parser.backlog.push(token);
            }
        }

        self.parse_body(&mut parser)?;

        Ok(parser)
    }

    pub fn parse_parameters(
        &mut self,
        parser: &mut ast::Parser,
    ) -> Result<(), Error> {
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
            let (direction, dtk) = self.parser.parse_direction()?;
            let (ty, ttk) = self.parser.parse_type()?;
            let (name, ntk) = self.parser.parse_identifier()?;
            let token = self.parser.next_token()?;
            if token.kind == lexer::Kind::ParenClose {
                parser.parameters.push(ControlParameter {
                    direction,
                    ty,
                    name,
                    dir_token: dtk,
                    ty_token: ttk,
                    name_token: ntk,
                });
                break;
            }
            self.parser.backlog.push(token.clone());
            self.parser.expect_token(lexer::Kind::Comma)?;

            parser.parameters.push(ControlParameter {
                direction,
                ty,
                name,
                dir_token: dtk,
                ty_token: ttk,
                name_token: ntk,
            });
        }

        Ok(())
    }

    pub fn parse_body(
        &mut self,
        parser: &mut ast::Parser,
    ) -> Result<(), Error> {
        self.parser.expect_token(lexer::Kind::CurlyOpen)?;

        // iterate over body statements
        loop {
            let token = self.parser.next_token()?;

            match token.kind {
                lexer::Kind::CurlyClose => break,
                lexer::Kind::State => self.parse_state(parser)?,
                _ => {
                    return Err(ParserError {
                        at: token.clone(),
                        message: format!(
                            "Found {} expected: state or nd of parser",
                            token.kind,
                        ),
                        source: self.parser.lexer.lines[token.line].into(),
                    }
                    .into())
                }
            }
        }

        Ok(())
    }

    pub fn parse_state(
        &mut self,
        parser: &mut ast::Parser,
    ) -> Result<(), Error> {
        let mut sp = StateParser::new(self.parser);
        let state = sp.run()?;
        parser.states.push(state);

        Ok(())
    }
}

pub struct StateParser<'a, 'b> {
    parser: &'b mut Parser<'a>,
}

impl<'a, 'b> StateParser<'a, 'b> {
    pub fn new(parser: &'b mut Parser<'a>) -> Self {
        Self { parser }
    }

    pub fn run(&mut self) -> Result<State, Error> {
        let (name, _) = self.parser.parse_identifier()?;
        let mut state = State::new(name);

        self.parse_body(&mut state)?;

        Ok(state)
    }

    pub fn parse_body(&mut self, state: &mut State) -> Result<(), Error> {
        state.statements = self.parser.parse_statement_block()?;
        Ok(())
    }
}

pub struct SelectParser<'a, 'b> {
    parser: &'b mut Parser<'a>,
}

impl<'a, 'b> SelectParser<'a, 'b> {
    pub fn new(parser: &'b mut Parser<'a>) -> Self {
        Self { parser }
    }

    pub fn run(&mut self) -> Result<Select, Error> {
        let mut select = Select {
            parameters: self.parser.parse_expr_parameters()?,
            ..Default::default()
        };
        self.parse_body(&mut select)?;
        Ok(select)
    }

    pub fn parse_body(&mut self, select: &mut Select) -> Result<(), Error> {
        self.parser.expect_token(lexer::Kind::CurlyOpen)?;

        loop {
            let token = self.parser.next_token()?;

            // check if we've reached the end of the parameters
            if token.kind == lexer::Kind::CurlyClose {
                break;
            }
            self.parser.backlog.push(token);

            let keyset = self.parser.parse_keyset()?;
            self.parser.expect_token(lexer::Kind::Colon)?;
            let (name, _) = self.parser.parse_identifier()?;
            self.parser.expect_token(lexer::Kind::Semicolon)?;
            select.elements.push(SelectElement { keyset, name });
        }

        Ok(())
    }
}
