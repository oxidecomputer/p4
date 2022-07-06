use std::collections::HashMap;
use crate::ast::{ 
    AST, BinOp, Constant, Control, DeclarationInfo, Expression, ExpressionKind,
    Lvalue, NameInfo, Parser, Statement, StatementBlock, Type,
};
use crate::check::{Diagnostics, Diagnostic, Level};
use crate::util::resolve_lvalue;

/// The P4 high level intermediate representation (hlir) is a slight lowering of
/// the abstract syntax tree (ast) into something a bit more concreate. In
/// particular
///
/// - Types are resolved for expressions.
/// - Lvalue names resolved to declarations.
///
/// The hlir maps language elements onto the corresponding type and declaration
/// information. Langauge elements contain lexical token members which ensure
/// hashing uniquenes.
#[derive(Debug, Default)]
pub struct Hlir {
    pub expression_types: HashMap::<Expression, Type>,
    pub lvalue_decls: HashMap::<Lvalue, DeclarationInfo>,
}


pub struct HlirGenerator<'a> { 
    ast: &'a AST,
    pub hlir: Hlir,
    pub diags: Diagnostics,
}

impl<'a> HlirGenerator<'a> {
    pub fn new(ast: &'a AST) -> Self {
        Self {
            ast,
            hlir: Hlir::default(),
            diags: Diagnostics::default(),
        }
    }
    pub fn run(&mut self) {
        for c in &self.ast.constants {
            self.constant(c);
        }
        for c in &self.ast.controls {
            self.control(c);
        }
        for p in &self.ast.parsers {
            self.parser(p);
        }
    }

    fn constant(&mut self, _c: &Constant) {
        // TODO
    }

    fn control(&mut self, c: &Control) {
        let mut names = c.names();
        for a in &c.actions {
            let mut local_names = names.clone();
            local_names.extend(a.names());
            self.statement_block(&a.statement_block, &mut local_names);
        }
        self.statement_block(&c.apply, &mut names);
    }

    fn statement_block(
        &mut self,
        sb: &StatementBlock,
        names: &mut HashMap::<String, NameInfo>
    ) {
        for stmt in &sb.statements {
            match stmt {
                Statement::Empty => {}
                Statement::Assignment(lval, xpr) => { 
                    self.lvalue(lval, names);
                    self.expression(xpr, names);
                }
                Statement::Call(c) => { 
                    self.lvalue(&c.lval, names);
                    for xpr in &c.args {
                        self.expression(xpr.as_ref(), names);
                    }
                }
                Statement::If(ifb) => { 
                    self.expression(ifb.predicate.as_ref(), names);
                    self.statement_block(&ifb.block, names);
                    for ei in &ifb.else_ifs {
                        self.expression(ei.predicate.as_ref(), names);
                        self.statement_block(&ei.block, names);
                    }
                    if let Some(eb) = &ifb.else_block {
                        self.statement_block(eb, names);
                    }
                }
                Statement::Variable(v) => {
                    names.insert(v.name.clone(), NameInfo{
                        ty: v.ty.clone(),
                        decl: DeclarationInfo::Local,
                    });
                    if let Some(initializer) = &v.initializer {
                        self.expression(initializer, names);
                    }
                }
                Statement::Constant(c) => {
                    names.insert(c.name.clone(), NameInfo{
                        ty: c.ty.clone(),
                        decl: DeclarationInfo::Local,
                    });
                    self.expression(c.initializer.as_ref(), names);
                }
            }
        }
    }

    fn expression(
        &mut self,
        xpr: &Expression,
        names: &mut HashMap::<String, NameInfo>
    ) -> Option<Type> {
        match &xpr.kind {
            ExpressionKind::BoolLit(_) => {
                let ty = Type::Bool;
                self.hlir.expression_types.insert(xpr.clone(), ty.clone());
                Some(ty)
            }
            ExpressionKind::IntegerLit(_) => {
                //TODO P4 spec section 8.9.1/8.9.2
                let ty = Type::Int(128);
                self.hlir.expression_types.insert(xpr.clone(), ty.clone());
                Some(ty)
            }
            ExpressionKind::BitLit(width, _) => {
                let ty = Type::Bit(*width as usize);
                self.hlir.expression_types.insert(xpr.clone(), ty.clone());
                Some(ty)
            }
            ExpressionKind::SignedLit(width, _) => {
                let ty = Type::Int(*width as usize);
                self.hlir.expression_types.insert(xpr.clone(), ty.clone());
                Some(ty)
            }
            ExpressionKind::Lvalue(lval) => {
                self.lvalue(lval, names)
            }
            ExpressionKind::Binary(lhs, op, rhs) => {
                self.binary_expression(xpr, lhs, rhs, op, names)
            }
            ExpressionKind::Index(lval, xpr) => {
                self.index(lval, xpr, names)
            }
            ExpressionKind::Slice(end, _begin) => {
                self.diags.push(Diagnostic{
                    level: Level::Error,
                    message: format!("slice cannot occur outside of an index"),
                    token: end.token.clone(),
                });
                None
            }
        }
    }

    fn index(
        &mut self,
        lval: &Lvalue,
        xpr: &Expression,
        names: &mut HashMap<String, NameInfo>
    ) -> Option<Type> {
        let base_type = match self.lvalue(lval, names) {
            Some(ty) => ty,
            None => return None
        };
        match base_type {
            Type::Bool => {
                self.diags.push(Diagnostic{
                    level: Level::Error,
                    message: format!("cannot index a bool"),
                    token: lval.token.clone(),
                });
                None
            }
            Type::Error => {
                self.diags.push(Diagnostic{
                    level: Level::Error,
                    message: format!("cannot index an error"),
                    token: lval.token.clone(),
                });
                None
            }
            Type::Bit(width) => {
                match &xpr.kind {
                    ExpressionKind::Slice(end, begin) => {
                        let (begin_val, end_val) = self.slice(begin, end, width)?;
                        let w = end_val - begin_val + 1;
                        Some(Type::Bit(w as usize))
                    }
                    _ => {
                        self.diags.push(Diagnostic{
                            level: Level::Error,
                            message: format!(
                                "only slices supported as index arguments"),
                            token: lval.token.clone(),
                        });
                        None
                    }
                }
            }
            Type::Varbit(width) => {
                match &xpr.kind {
                    ExpressionKind::Slice(begin, end) => {
                        let (begin_val, end_val) = self.slice(begin, end, width)?;
                        let w = end_val - begin_val + 1;
                        Some(Type::Varbit(w as usize))
                    }
                    _ => {
                        self.diags.push(Diagnostic{
                            level: Level::Error,
                            message: format!(
                                "only slices supported as index arguments"),
                            token: lval.token.clone(),
                        });
                        None
                    }
                }
            }
            Type::Int(width) => {
                match &xpr.kind {
                    ExpressionKind::Slice(begin, end) => {
                        let (begin_val, end_val) = self.slice(begin, end, width)?;
                        let w = end_val - begin_val + 1;
                        Some(Type::Int(w as usize))
                    }
                    _ => {
                        self.diags.push(Diagnostic{
                            level: Level::Error,
                            message: format!(
                                "only slices supported as index arguments"),
                            token: lval.token.clone(),
                        });
                        None
                    }
                }
            }
            Type::String => {
                self.diags.push(Diagnostic{
                    level: Level::Error,
                    message: format!("cannot index a string"),
                    token: lval.token.clone(),
                });
                None
            }
            Type::UserDefined(_) => {
                self.diags.push(Diagnostic{
                    level: Level::Error,
                    message: format!("cannot index a user defined type"),
                    token: lval.token.clone(),
                });
                None
            }
            Type::ExternFunction => {
                self.diags.push(Diagnostic{
                    level: Level::Error,
                    message: format!("cannot index an external function"),
                    token: lval.token.clone(),
                });
                None
            }
            Type::Table => {
                self.diags.push(Diagnostic{
                    level: Level::Error,
                    message: format!("cannot index a table"),
                    token: lval.token.clone(),
                });
                None
            }
        }
    }

    fn slice(
        &mut self,
        begin: &Expression,
        end: &Expression,
        width: usize,
    ) -> Option<(i128, i128)> {
        // According to P4-16 section 8.5, slice values must be
        // known at compile time. For now just enfoce integer
        // literals only, we can get fancier later with other
        // things that can be figured out at compile time.
        let begin_val = match &begin.kind {
            ExpressionKind::IntegerLit(v) => *v,
            _ => {
                self.diags.push(Diagnostic{
                    level: Level::Error,
                    message: format!(
                        "only interger literals are supported as slice bounds"
                    ),
                    token: begin.token.clone(),
                });
                return None
            }
        };
        let end_val = match &end.kind {
            ExpressionKind::IntegerLit(v) => *v,
            _ => {
                self.diags.push(Diagnostic{
                    level: Level::Error,
                    message: format!(
                        "only interger literals are supported as slice bounds"
                    ),
                    token: begin.token.clone(),
                });
                return None
            }
        };
        let w = width as i128;
        if begin_val < 0 || begin_val >= w {
            self.diags.push(Diagnostic{
                level: Level::Error,
                message: format!("slice begin value out of bounds"),
                token: begin.token.clone(),
            });
            return None
        }
        if end_val < 0 || end_val >= w {
            self.diags.push(Diagnostic{
                level: Level::Error,
                message: format!("slice end value out of bounds"),
                token: begin.token.clone(),
            });
            return None
        }
        if begin_val >= end_val {
            self.diags.push(Diagnostic{
                level: Level::Error,
                message: format!("slice upper bound must be \
                    greater than the lower bound"),
                    token: begin.token.clone(),
            });
            return None
        }
        Some((begin_val, end_val))
    }


    fn lvalue(
        &mut self,
        lval: &Lvalue,
        names: &mut HashMap<String, NameInfo>
    ) -> Option<Type> {
        match resolve_lvalue(lval, self.ast, &names) {
            Ok(name_info) => {
                self.hlir.lvalue_decls.insert(
                    lval.clone(),
                    name_info.decl,
                );
                Some(name_info.ty)
            }
            Err(_e) => {
                self.diags.push(Diagnostic {
                    level: Level::Error,
                    message: format!(
                        "could not resolve lvalue: {}",
                        lval.name,
                    ),
                    token: lval.token.clone(),
                });
                None
            }
        }
    }

    fn binary_expression(
        &mut self,
        xpr: &Expression,
        lhs: &Expression,
        rhs: &Expression,
        op: &BinOp,
        names: &mut HashMap<String, NameInfo>
    ) -> Option<Type> {
        let lhs_ty = match self.expression(lhs, names) {
            Some(ty) => ty,
            None => return None,
        };

        let rhs_ty = match self.expression(rhs, names) {
            Some(ty) => ty,
            None => return None,
        };

        // TODO just checking that types are the same for now.
        if lhs_ty != rhs_ty {
            self.diags.push(Diagnostic {
                level: Level::Error,
                message: format!(
                    "cannot {} a {} and a {}",
                    op.english_verb(),
                    lhs_ty,
                    rhs_ty,
                ),
                token: xpr.token.clone(),
            });
        }

        self.hlir.expression_types.insert(xpr.clone(), lhs_ty.clone());
        Some(lhs_ty)

    }

    fn parser(&mut self, _p: &Parser) {
        // TODO
    }
}
