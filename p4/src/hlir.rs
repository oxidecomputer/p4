// Copyright 2022 Oxide Computer Company

use crate::ast::{
    BinOp, Constant, Control, DeclarationInfo, Expression, ExpressionKind,
    Lvalue, NameInfo, Parser, Statement, StatementBlock, Type, AST,
};
use crate::check::{Diagnostic, Diagnostics, Level};
use crate::util::resolve_lvalue;
use std::collections::HashMap;

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
    pub expression_types: HashMap<Expression, Type>,
    pub lvalue_decls: HashMap<Lvalue, NameInfo>,
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
        for t in &c.tables {
            let mut local_names = names.clone();
            for (lval, _match_kind) in &t.key {
                self.lvalue(lval, &mut local_names);
            }
            for lval in &t.actions {
                self.lvalue(lval, &mut local_names);
            }
        }
        self.statement_block(&c.apply, &mut names);
    }

    fn statement_block(
        &mut self,
        sb: &StatementBlock,
        names: &mut HashMap<String, NameInfo>,
    ) {
        for stmt in &sb.statements {
            match stmt {
                Statement::Empty => {}
                Statement::Assignment(lval, xpr) => {
                    self.lvalue(lval, names);
                    self.expression(xpr, names);
                }
                Statement::Call(c) => {
                    // pop the function name off the lval before resolving
                    self.lvalue(&c.lval.pop_right(), names);
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
                    names.insert(
                        v.name.clone(),
                        NameInfo {
                            ty: v.ty.clone(),
                            decl: DeclarationInfo::Local,
                        },
                    );
                    if let Some(initializer) = &v.initializer {
                        self.expression(initializer, names);
                    }
                }
                Statement::Constant(c) => {
                    names.insert(
                        c.name.clone(),
                        NameInfo {
                            ty: c.ty.clone(),
                            decl: DeclarationInfo::Local,
                        },
                    );
                    self.expression(c.initializer.as_ref(), names);
                }
                Statement::Transition(_t) => {
                    //TODO
                }
                Statement::Return(xpr) => {
                    if let Some(xpr) = xpr {
                        self.expression(xpr.as_ref(), names);
                    }
                }
            }
        }
    }

    fn expression(
        &mut self,
        xpr: &Expression,
        names: &mut HashMap<String, NameInfo>,
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
                let ty = self.lvalue(lval, names)?;
                self.hlir.expression_types.insert(xpr.clone(), ty.clone());
                Some(ty)
            }
            ExpressionKind::Binary(lhs, op, rhs) => {
                self.binary_expression(xpr, lhs, rhs, op, names)
            }
            ExpressionKind::Index(lval, i_xpr) => {
                if let Some(ty) = self.index(lval, i_xpr, names) {
                    self.hlir.expression_types.insert(xpr.clone(), ty.clone());
                    Some(ty)
                } else {
                    None
                }
            }
            ExpressionKind::Slice(end, _begin) => {
                self.diags.push(Diagnostic {
                    level: Level::Error,
                    message: "slice cannot occur outside of an index".into(),
                    token: end.token.clone(),
                });
                None
            }
            ExpressionKind::Call(call) => {
                self.lvalue(&call.lval.pop_right(), names)?;
                for arg in &call.args {
                    self.expression(arg.as_ref(), names);
                }
                // TODO check extern methods in checker before getting here
                if let Some(name_info) = names.get(call.lval.root()) {
                    if let Type::UserDefined(typename) = &name_info.ty {
                        if let Some(ext) = self.ast.get_extern(typename) {
                            if let Some(m) = ext.get_method(call.lval.leaf()) {
                                self.hlir
                                    .expression_types
                                    .insert(xpr.clone(), m.return_type.clone());
                                return Some(m.return_type.clone());
                            }
                        }
                    }
                };
                //TODO less special case-y?
                Some(match call.lval.leaf() {
                    "isValid" => Type::Bool,
                    _ => Type::Void,
                })
            }
            ExpressionKind::List(elements) => {
                let mut type_elements = Vec::new();
                for e in elements {
                    let ty = match self.expression(e.as_ref(), names) {
                        Some(ty) => ty,
                        None => return None,
                    };
                    type_elements.push(Box::new(ty));
                }
                Some(Type::List(type_elements))
            }
        }
    }

    fn index(
        &mut self,
        lval: &Lvalue,
        xpr: &Expression,
        names: &mut HashMap<String, NameInfo>,
    ) -> Option<Type> {
        let base_type = match self.lvalue(lval, names) {
            Some(ty) => ty,
            None => return None,
        };
        match base_type {
            Type::Bool => {
                self.diags.push(Diagnostic {
                    level: Level::Error,
                    message: "cannot index a bool".into(),
                    token: lval.token.clone(),
                });
                None
            }
            Type::State => {
                self.diags.push(Diagnostic {
                    level: Level::Error,
                    message: "cannot index a state".into(),
                    token: lval.token.clone(),
                });
                None
            }
            Type::Action => {
                self.diags.push(Diagnostic {
                    level: Level::Error,
                    message: "cannot index an action".into(),
                    token: lval.token.clone(),
                });
                None
            }
            Type::Error => {
                self.diags.push(Diagnostic {
                    level: Level::Error,
                    message: "cannot index an error".into(),
                    token: lval.token.clone(),
                });
                None
            }
            Type::Void => {
                self.diags.push(Diagnostic {
                    level: Level::Error,
                    message: "cannot index a void".into(),
                    token: lval.token.clone(),
                });
                None
            }
            Type::List(_) => {
                self.diags.push(Diagnostic {
                    level: Level::Error,
                    message: "cannot index a list".into(),
                    token: lval.token.clone(),
                });
                None
            }
            Type::Bit(width) => match &xpr.kind {
                ExpressionKind::Slice(end, begin) => {
                    let (begin_val, end_val) = self.slice(begin, end, width)?;
                    let w = end_val - begin_val + 1;
                    Some(Type::Bit(w as usize))
                }
                _ => {
                    self.diags.push(Diagnostic {
                        level: Level::Error,
                        message: "only slices supported as index arguments"
                            .into(),
                        token: lval.token.clone(),
                    });
                    None
                }
            },
            Type::Varbit(width) => match &xpr.kind {
                ExpressionKind::Slice(begin, end) => {
                    let (begin_val, end_val) = self.slice(begin, end, width)?;
                    let w = end_val - begin_val + 1;
                    Some(Type::Varbit(w as usize))
                }
                _ => {
                    self.diags.push(Diagnostic {
                        level: Level::Error,
                        message: "only slices supported as index arguments"
                            .into(),
                        token: lval.token.clone(),
                    });
                    None
                }
            },
            Type::Int(width) => match &xpr.kind {
                ExpressionKind::Slice(begin, end) => {
                    let (begin_val, end_val) = self.slice(begin, end, width)?;
                    let w = end_val - begin_val + 1;
                    Some(Type::Int(w as usize))
                }
                _ => {
                    self.diags.push(Diagnostic {
                        level: Level::Error,
                        message: "only slices supported as index arguments"
                            .into(),
                        token: lval.token.clone(),
                    });
                    None
                }
            },
            Type::String => {
                self.diags.push(Diagnostic {
                    level: Level::Error,
                    message: "cannot index a string".into(),
                    token: lval.token.clone(),
                });
                None
            }
            Type::UserDefined(_) => {
                self.diags.push(Diagnostic {
                    level: Level::Error,
                    message: "cannot index a user defined type".into(),
                    token: lval.token.clone(),
                });
                None
            }
            Type::ExternFunction => {
                self.diags.push(Diagnostic {
                    level: Level::Error,
                    message: "cannot index an external function".into(),
                    token: lval.token.clone(),
                });
                None
            }
            Type::Table => {
                self.diags.push(Diagnostic {
                    level: Level::Error,
                    message: "cannot index a table".into(),
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
                self.diags.push(Diagnostic {
                    level: Level::Error,
                    message:
                        "only interger literals are supported as slice bounds"
                            .into(),
                    token: begin.token.clone(),
                });
                return None;
            }
        };
        let end_val = match &end.kind {
            ExpressionKind::IntegerLit(v) => *v,
            _ => {
                self.diags.push(Diagnostic {
                    level: Level::Error,
                    message:
                        "only interger literals are supported as slice bounds"
                            .into(),
                    token: begin.token.clone(),
                });
                return None;
            }
        };
        let w = width as i128;
        if begin_val < 0 || begin_val >= w {
            self.diags.push(Diagnostic {
                level: Level::Error,
                message: "slice begin value out of bounds".into(),
                token: begin.token.clone(),
            });
            return None;
        }
        if end_val < 0 || end_val >= w {
            self.diags.push(Diagnostic {
                level: Level::Error,
                message: "slice end value out of bounds".into(),
                token: begin.token.clone(),
            });
            return None;
        }
        if begin_val >= end_val {
            self.diags.push(Diagnostic {
                level: Level::Error,
                message: "slice upper bound must be \
                    greater than the lower bound"
                    .into(),
                token: begin.token.clone(),
            });
            return None;
        }
        Some((begin_val, end_val))
    }

    fn lvalue(
        &mut self,
        lval: &Lvalue,
        names: &mut HashMap<String, NameInfo>,
    ) -> Option<Type> {
        match resolve_lvalue(lval, self.ast, names) {
            Ok(name_info) => {
                self.hlir
                    .lvalue_decls
                    .insert(lval.clone(), name_info.clone());
                Some(name_info.ty)
            }
            Err(e) => {
                self.diags.push(Diagnostic {
                    level: Level::Error,
                    message: format!(
                        "could not resolve lvalue: {}\n    {}",
                        lval.name, e,
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
        names: &mut HashMap<String, NameInfo>,
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

        self.hlir
            .expression_types
            .insert(xpr.clone(), lhs_ty.clone());
        Some(lhs_ty)
    }

    fn parser(&mut self, p: &Parser) {
        let names = p.names();
        for s in &p.states {
            let mut local_names = names.clone();
            self.statement_block(&s.statements, &mut local_names);
        }
    }
}
