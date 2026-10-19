// Copyright 2026 Oxide Computer Company

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
                if lval.name != "NoAction" {
                    self.lvalue(lval, &mut local_names);
                }
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
                    let target = self.lvalue(lval, names);
                    let actual = self.expression(xpr, names);
                    if let (Some(target), Some(actual)) = (target, actual) {
                        if !implicitly_converts(xpr, &actual, &target) {
                            let message = if unsupported_int_conversion(
                                xpr, &actual, &target,
                            ) {
                                UNSUPPORTED_INT_EXPRESSION.into()
                            } else {
                                format!("cannot assign {actual} to {target}")
                            };
                            self.diags.push(Diagnostic {
                                level: Level::Error,
                                message,
                                token: xpr.token.clone(),
                            });
                        }
                    }
                }
                Statement::SliceAssignment(lval, hi, lo, xpr) => {
                    self.lvalue(lval, names);
                    self.expression(hi, names);
                    self.expression(lo, names);
                    let expression_type = self.expression(xpr, names);

                    // Validate slice bounds.
                    if let Some(name_info) = self.hlir.lvalue_decls.get(lval) {
                        let width = match &name_info.ty {
                            Type::Bit(w) => *w,
                            ty => {
                                let message = match ty {
                                    Type::Int(_) => format!(
                                        "slice assignment to {ty} is not \
                                         supported by SoftNPU"
                                    ),
                                    _ => format!(
                                        "slice assignment requires a bit \
                                         type, got {ty}"
                                    ),
                                };
                                self.diags.push(Diagnostic {
                                    level: Level::Error,
                                    message,
                                    token: lval.token.clone(),
                                });
                                continue;
                            }
                        };
                        if let (Some((hi_val, lo_val)), Some(expression_type)) =
                            (self.slice_bounds(hi, lo, width), expression_type)
                        {
                            // P4-16 8.6: a slice is a bit<H-L+1> l-value.
                            let expected_ty =
                                Type::Bit((hi_val - lo_val + 1) as usize);

                            if !implicitly_converts(
                                xpr,
                                &expression_type,
                                &expected_ty,
                            ) {
                                let message = if unsupported_int_conversion(
                                    xpr,
                                    &expression_type,
                                    &expected_ty,
                                ) {
                                    UNSUPPORTED_INT_EXPRESSION.into()
                                } else {
                                    format!(
                                        "slice [{hi_val}:{lo_val}] requires \
                                         {expected_ty}, got {expression_type}"
                                    )
                                };

                                self.diags.push(Diagnostic {
                                    level: Level::Error,
                                    message,
                                    token: xpr.token.clone(),
                                });
                            }
                        }
                    }
                }
                Statement::Call(c) => {
                    // pop the function name off the lval before resolving
                    self.lvalue(&c.lval.pop_right(), names);
                    for xpr in &c.args {
                        self.expression(xpr.as_ref(), names);
                    }
                }
                Statement::If(ifb) => {
                    self.predicate(ifb.predicate.as_ref(), names);
                    self.statement_block(&ifb.block, names);
                    for ei in &ifb.else_ifs {
                        self.predicate(ei.predicate.as_ref(), names);
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
                        self.initializer(&v.ty, initializer, names);
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
                    self.initializer(&c.ty, c.initializer.as_ref(), names);
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

    /// Check a declaration's initializer against its declared type.
    ///
    /// This uses `implicitly_converts`, reporting compile-time `int` expressions
    /// other than literals as an x4c limitation rather than a type error.
    fn initializer(
        &mut self,
        declared: &Type,
        xpr: &Expression,
        names: &mut HashMap<String, NameInfo>,
    ) {
        let Some(actual) = self.expression(xpr, names) else {
            return;
        };

        if !implicitly_converts(xpr, &actual, declared) {
            let message = if unsupported_int_conversion(xpr, &actual, declared)
            {
                UNSUPPORTED_INT_EXPRESSION.into()
            } else {
                format!("cannot initialize {declared} with {actual}")
            };
            self.diags.push(Diagnostic {
                level: Level::Error,
                message,
                token: xpr.token.clone(),
            });
        }
    }

    fn literal(&mut self, xpr: &Expression, ty: Type) -> Option<Type> {
        if let Type::Bit(w) | Type::Int(w) = ty {
            if w > 128 {
                self.diags.push(Diagnostic {
                    level: Level::Error,
                    message: format!(
                        "Width {w} exceeds the 128-bit compiler limit"
                    ),
                    token: xpr.token.clone(),
                });
                return None;
            }
        }
        self.hlir.expression_types.insert(xpr.clone(), ty.clone());
        Some(ty)
    }

    fn predicate(
        &mut self,
        xpr: &Expression,
        names: &mut HashMap<String, NameInfo>,
    ) {
        let Some(ty) = self.expression(xpr, names) else {
            return;
        };

        if ty != Type::Bool {
            self.diags.push(Diagnostic {
                level: Level::Error,
                message: format!("condition must be bool, got {ty}"),
                token: xpr.token.clone(),
            });
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
                //TODO P4 spec sections 6.4.3.3 (integer literals) and 8.11.2
                // (implicit int casts)
                let ty = Type::Int(128);
                self.hlir.expression_types.insert(xpr.clone(), ty.clone());
                Some(ty)
            }
            ExpressionKind::BitLit(width, _) => {
                self.literal(xpr, Type::Bit(*width as usize))
            }
            ExpressionKind::SignedLit(width, _) => {
                self.literal(xpr, Type::Int(*width as usize))
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
                let ty = match call.lval.leaf() {
                    "isValid" => Type::Bool,
                    _ => Type::Void,
                };
                self.hlir.expression_types.insert(xpr.clone(), ty.clone());
                Some(ty)
            }
            ExpressionKind::List(elements) => {
                let mut type_elements = Vec::new();
                for e in elements {
                    let ty = self.expression(e.as_ref(), names)?;
                    type_elements.push(Box::new(ty));
                }
                let ty = Type::List(type_elements);
                self.hlir.expression_types.insert(xpr.clone(), ty.clone());
                Some(ty)
            }
        }
    }

    fn index(
        &mut self,
        lval: &Lvalue,
        xpr: &Expression,
        names: &mut HashMap<String, NameInfo>,
    ) -> Option<Type> {
        let base_type = self.lvalue(lval, names)?;
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
            // P4-16 8.6: a slice is an unsigned bit<H-L+1>.
            Type::Bit(width) => match &xpr.kind {
                ExpressionKind::Slice(hi, lo) => {
                    let (hi, lo) = self.slice_bounds(hi, lo, width)?;
                    Some(Type::Bit((hi - lo + 1) as usize))
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
            // TODO: P4-16 8.7 and 8.8 allow slicing int<W> and int.
            // SoftNPU, on the other hand, has no int<W> code generation, and
            // x4c does not evaluate compile-time int expressions.
            Type::Int(w) => {
                self.diags.push(Diagnostic {
                    level: Level::Error,
                    message: format!(
                        "slicing int<{w}> is not supported by SoftNPU"
                    ),
                    token: lval.token.clone(),
                });
                None
            }
            Type::ArbitraryInt => {
                self.diags.push(Diagnostic {
                    level: Level::Error,
                    message: "slicing a compile-time int is not supported by \
                              x4c"
                    .into(),
                    token: lval.token.clone(),
                });
                None
            }
            // P4-16 8.10: varbit supports only assignment and ==/!=.
            Type::Varbit(_) => {
                self.diags.push(Diagnostic {
                    level: Level::Error,
                    message: "cannot index a varbit".into(),
                    token: lval.token.clone(),
                });
                None
            }
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
            Type::HeaderMethod => {
                self.diags.push(Diagnostic {
                    level: Level::Error,
                    message: "cannot index a header method".into(),
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

    /// Validate a slice `[hi:lo]` on a field of `width` bits.
    fn slice_bounds(
        &mut self,
        hi: &Expression,
        lo: &Expression,
        width: usize,
    ) -> Option<(i128, i128)> {
        // P4-16 section 8.6: slice bounds must be compile-time
        // known values. Only integer literals are accepted; constant
        // expressions are not yet supported.
        let hi_val = self.slice_bound(hi, width, "upper")?;
        let lo_val = self.slice_bound(lo, width, "lower")?;
        if hi_val < lo_val {
            self.diags.push(Diagnostic {
                level: Level::Error,
                message: "slice upper bound must be \
                    greater than or equal to the lower bound"
                    .into(),
                token: hi.token.clone(),
            });
            return None;
        }
        Some((hi_val, lo_val))
    }

    fn slice_bound(
        &mut self,
        bound: &Expression,
        width: usize,
        which: &str,
    ) -> Option<i128> {
        let ExpressionKind::IntegerLit(v) = bound.kind else {
            self.diags.push(Diagnostic {
                level: Level::Error,
                message: "only integer literals are supported by x4c as \
                          slice bounds"
                    .into(),
                token: bound.token.clone(),
            });
            return None;
        };

        if !(0..width as i128).contains(&v) {
            self.diags.push(Diagnostic {
                level: Level::Error,
                message: format!("slice {which} bound out of bounds"),
                token: bound.token.clone(),
            });
            return None;
        }
        Some(v)
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

    /// Type the binary expression or report an error and return `None`.
    ///
    /// Shifts need a `bit<W>` left operand and a `bit<W>` or non-negative
    /// literal count.
    ///
    /// Every other operator needs matching operand types, and
    /// ordered comparisons also need `bit<W>` operands. Comparisons produce
    /// `bool`, while other operators produce the left operand's type.
    fn binary_expression(
        &mut self,
        xpr: &Expression,
        lhs: &Expression,
        rhs: &Expression,
        op: &BinOp,
        names: &mut HashMap<String, NameInfo>,
    ) -> Option<Type> {
        let lhs_ty = self.expression(lhs, names)?;
        let rhs_ty = self.expression(rhs, names)?;
        let valid_count = matches!(&rhs_ty, Type::Bit(_))
            || matches!(
                &rhs.kind,
                ExpressionKind::IntegerLit(value) if *value >= 0
            );

        // TODO arithmetic and bitwise operands are only checked to have the
        // same type; P4-16 8.6 limits them to bit and int types.
        match op {
            BinOp::Shl | BinOp::Shr if !valid_count => {
                self.diags.push(Diagnostic {
                    level: Level::Error,
                    message: "shift count must be a bit type or a \
                        non-negative integer literal"
                        .into(),
                    token: rhs.token.clone(),
                });
                return None;
            }
            BinOp::Shl | BinOp::Shr if !matches!(&lhs_ty, Type::Bit(_)) => {
                let message = match &lhs_ty {
                    ty if compile_time_int(lhs, ty) => {
                        "shifting a compile-time int is not supported by x4c"
                            .into()
                    }
                    Type::Int(_) => {
                        format!("shifting {lhs_ty} is not supported by SoftNPU")
                    }
                    _ => format!(
                        "shift left operand must be a bit type, got {lhs_ty}"
                    ),
                };
                self.diags.push(Diagnostic {
                    level: Level::Error,
                    message,
                    token: lhs.token.clone(),
                });
                return None;
            }
            BinOp::Shl | BinOp::Shr => {}
            BinOp::Lt | BinOp::Gt | BinOp::Leq | BinOp::Geq
                if !matches!(&lhs_ty, Type::Bit(_)) =>
            {
                let non_integer = [(lhs, &lhs_ty), (rhs, &rhs_ty)]
                    .into_iter()
                    .find(|(_, ty)| !integer_type(ty));
                let (message, token) = match non_integer {
                    Some((operand, ty)) => (
                        format!(
                            "ordered comparison requires bit operands, got \
                             {ty}"
                        ),
                        operand.token.clone(),
                    ),
                    None if compile_time_int(lhs, &lhs_ty) => (
                        "ordered comparison of a compile-time int is not \
                         supported by x4c"
                            .into(),
                        lhs.token.clone(),
                    ),
                    None => (
                        format!(
                            "ordered comparison of {lhs_ty} is not supported \
                             by SoftNPU"
                        ),
                        lhs.token.clone(),
                    ),
                };
                self.diags.push(Diagnostic {
                    level: Level::Error,
                    message,
                    token,
                });
                return None;
            }
            _ if lhs_ty != rhs_ty => {
                self.diags.push(Diagnostic {
                    level: Level::Error,
                    message: format!(
                        "cannot {} a {lhs_ty} and a {rhs_ty}",
                        op.english_verb(),
                    ),
                    token: xpr.token.clone(),
                });
                return None;
            }
            _ => {}
        }

        // P4-16 8.6: comparisons produce a Boolean value.
        let ty = match op {
            BinOp::Eq
            | BinOp::NotEq
            | BinOp::Lt
            | BinOp::Gt
            | BinOp::Leq
            | BinOp::Geq => Type::Bool,
            _ => lhs_ty,
        };
        self.hlir.expression_types.insert(xpr.clone(), ty.clone());
        Some(ty)
    }

    fn parser(&mut self, p: &Parser) {
        let names = p.names();
        for s in &p.states {
            let mut local_names = names.clone();
            self.statement_block(&s.statements, &mut local_names);
        }
    }
}

/// Whether a value of type `from` may be used where `to` is expected or
/// asserted.
///
/// P4-16 [8.11.2] allows an arbitrary-precision `int` to convert implicitly
/// to `bit<W>`, but only integer literals are accepted here, while
/// compile-time `int` expressions such as `1 + 2` are not.
///
/// x4c also accepts implicit widening from `bit<W>` to `bit<V>` when
/// `W < V`, zero-extending the value.
/// P4-16 [8.11.1] and [8.11.2] require an explicit cast between different bit
/// widths; x4c does not currently parse explicit casts.
///
/// [8.11.1]: https://p4.org/wp-content/uploads/sites/53/2024/10/P4-16-spec-v1.2.5.html#sec-explicit-casts
/// [8.11.2]: https://p4.org/wp-content/uploads/sites/53/2024/10/P4-16-spec-v1.2.5.html#sec-implicit-casts
pub(crate) fn implicitly_converts(
    xpr: &Expression,
    from: &Type,
    to: &Type,
) -> bool {
    from == to
        || matches!(
            (&xpr.kind, to),
            (ExpressionKind::IntegerLit(_), Type::Bit(_))
        )
        || matches!((from, to), (Type::Bit(w), Type::Bit(v)) if w <= v)
}

const UNSUPPORTED_INT_EXPRESSION: &str =
    "int expressions other than literals are not supported by x4c";

fn literal_int_expression(xpr: &Expression) -> bool {
    match &xpr.kind {
        ExpressionKind::IntegerLit(_) => true,
        ExpressionKind::Binary(lhs, _, rhs) => {
            literal_int_expression(lhs) && literal_int_expression(rhs)
        }
        _ => false,
    }
}

/// Whether `xpr` is a compile-time `int`: typed `int`, or an integer-typed
/// expression built only from integer literals.
fn compile_time_int(xpr: &Expression, ty: &Type) -> bool {
    matches!(ty, Type::ArbitraryInt)
        || (matches!(ty, Type::Int(_)) && literal_int_expression(xpr))
}

fn integer_type(ty: &Type) -> bool {
    matches!(ty, Type::Bit(_) | Type::Int(_) | Type::ArbitraryInt)
}

/// Whether a failed conversion to `bit<W>` comes from a compile-time `int`
/// expression that's not literal, which x4c cannot evaluate.
fn unsupported_int_conversion(
    xpr: &Expression,
    from: &Type,
    to: &Type,
) -> bool {
    !matches!(xpr.kind, ExpressionKind::IntegerLit(_))
        && compile_time_int(xpr, from)
        && matches!(to, Type::Bit(_))
}

#[cfg(test)]
mod tests {
    use crate::ast::{Expression, ExpressionKind, Statement, Type, AST};
    use crate::lexer::Lexer;
    use crate::parser::Parser;
    use std::sync::Arc;

    fn parse_p4(source: &str) -> AST {
        let lines: Vec<&str> = source.lines().collect();
        let filename = Arc::new("test.p4".to_string());
        let lexer = Lexer::new(lines, filename);
        let mut parser = Parser::new(lexer);
        let mut ast = AST::default();
        parser.run(&mut ast).expect("parse failed");
        ast
    }

    fn check_p4(source: &str) -> crate::check::Diagnostics {
        let ast = parse_p4(source);
        let (_hlir, diags) = crate::check::all(&ast);
        diags
    }

    fn assert_slice_error(stmt: &str, bound: &str, message: &str) {
        let source = format!(
            r#"
header h_t {{
    bit<32> f;
}}
struct headers_t {{
    h_t h;
}}
control ingress(inout headers_t hdr) {{
    apply {{
        bit<32> n = 32w3;
        {stmt}
    }}
}}
"#
        );
        let line = source
            .lines()
            .position(|l| l.contains(stmt))
            .expect("statement line");
        let col = source.lines().nth(line).unwrap().find(bound).unwrap();

        let diags = check_p4(&source);
        let errors = diags.errors();

        assert_eq!(
            errors.len(),
            1,
            "expected one error for `{stmt}`, got {:?}",
            errors.iter().map(|d| &d.message).collect::<Vec<_>>(),
        );
        assert_eq!(errors[0].message, message, "message for `{stmt}`");
        assert_eq!(
            (errors[0].token.line, errors[0].token.col),
            (line, col),
            "token position for `{stmt}`",
        );
    }

    const NON_LITERAL: &str =
        "only integer literals are supported by x4c as slice bounds";
    const HI_OOB: &str = "slice upper bound out of bounds";
    const LO_OOB: &str = "slice lower bound out of bounds";
    const HI_LT_LO: &str =
        "slice upper bound must be greater than or equal to the lower bound";

    #[test]
    fn slice_read_bound_errors() {
        assert_slice_error("bit<8> x = hdr.h.f[n:0];", "n:0]", NON_LITERAL);
        assert_slice_error("bit<8> x = hdr.h.f[32:0];", "32:0]", HI_OOB);
        assert_slice_error("bit<8> x = hdr.h.f[31:32];", "32]", LO_OOB);
        assert_slice_error("bit<8> x = hdr.h.f[3:7];", "3:7]", HI_LT_LO);
    }

    #[test]
    fn slice_assign_bound_errors() {
        assert_slice_error("hdr.h.f[0:n] = 1w0;", "n]", NON_LITERAL);
        assert_slice_error("hdr.h.f[32:0] = 1w0;", "32:0]", HI_OOB);
        assert_slice_error("hdr.h.f[31:32] = 1w0;", "32]", LO_OOB);
        assert_slice_error("hdr.h.f[3:7] = 1w0;", "3:7]", HI_LT_LO);
    }

    const SLICE_ASSIGN_CONTEXTS: &[&str] = &[
        "apply { STATEMENT }",
        "apply { if (false) { } else if (true) { STATEMENT } }",
        "action assign() { STATEMENT } apply { }",
    ];

    fn slice_assign_source(context: &str, slice: &str, rhs: &str) -> String {
        let body =
            context.replace("STATEMENT", &format!("hdr.h.f{slice} = {rhs};"));
        format!(
            r#"
header h_t {{
    bit<32> f;
}}
struct headers_t {{
    h_t h;
}}
control ingress(inout headers_t hdr) {{
    {body}
}}
"#
        )
    }

    #[test]
    fn slice_assign_rhs_type_errors() {
        for context in SLICE_ASSIGN_CONTEXTS {
            for (rhs, rhs_type) in
                [("16w1", "bit<16>"), ("8s1", "int<8>"), ("true", "bool")]
            {
                let source = slice_assign_source(context, "[7:0]", rhs);
                let rhs_start = format!("= {rhs};");
                let (line, text) = source
                    .lines()
                    .enumerate()
                    .find(|(_, text)| text.contains(&rhs_start))
                    .unwrap();
                let col = text.find(&rhs_start).unwrap() + 2;

                let diags = check_p4(&source);
                let errors = diags.errors();
                assert_eq!(
                    errors.len(),
                    1,
                    "{context}, {rhs}: {:?}",
                    errors.iter().map(|d| &d.message).collect::<Vec<_>>(),
                );
                assert_eq!(
                    errors[0].message,
                    format!("slice [7:0] requires bit<8>, got {rhs_type}"),
                    "{context}, {rhs}",
                );
                assert_eq!(
                    (errors[0].token.line, errors[0].token.col),
                    (line, col),
                    "{context}, {rhs}",
                );
            }
        }
    }

    #[test]
    fn slice_assign_rhs_matching_types() {
        for context in SLICE_ASSIGN_CONTEXTS {
            for rhs in ["8w1", "4w1", "1", "hdr.h.f[15:8]"] {
                let source = slice_assign_source(context, "[7:0]", rhs);
                let diags = check_p4(&source);
                let errors = diags.errors();
                assert!(
                    errors.is_empty(),
                    "{context}, {rhs}: {:?}",
                    errors.iter().map(|d| &d.message).collect::<Vec<_>>(),
                );
            }
        }
    }

    fn errors_for(source: &str) -> Vec<String> {
        check_p4(source)
            .errors()
            .iter()
            .map(|d| d.message.clone())
            .collect()
    }

    #[test]
    fn slice_assign_rejects_comparison_rhs() {
        let source = slice_assign_source(
            SLICE_ASSIGN_CONTEXTS[0],
            "[7:0]",
            "hdr.h.f[7:0] == hdr.h.f[15:8]",
        );
        assert_eq!(
            errors_for(&source),
            ["slice [7:0] requires bit<8>, got bool"]
        );
    }

    #[test]
    fn varbit_index_rejected() {
        let source = r#"
header h_t {
    varbit<32> v;
}
struct headers_t {
    h_t h;
}
control ingress(inout headers_t hdr) {
    apply {
        bit<8> x = hdr.h.v[7:0];
    }
}
"#;
        assert_eq!(errors_for(source), ["cannot index a varbit"]);
    }

    #[test]
    fn action_assignment_follows_apply_rules() {
        let source = r#"
header h_t {
    bit<8> f;
}
struct headers_t {
    h_t h;
}
control ingress(inout headers_t hdr) {
    action set() {
        hdr.h.f = 5;
    }
    action wide() {
        hdr.h.f = 16w1;
    }
    apply { }
}
"#;
        assert_eq!(errors_for(source), ["cannot assign bit<16> to bit<8>"]);
    }

    fn statement_source(statement: &str) -> String {
        format!(
            r#"
control ingress() {{
    apply {{
        bit<8> x = 8w1;
        {statement}
    }}
}}
"#
        )
    }

    #[test]
    fn initializer_type_errors() {
        for (statement, message) in [
            ("bit<4> y = x;", "cannot initialize bit<4> with bit<8>"),
            ("bool y = x;", "cannot initialize bool with bit<8>"),
            ("if (x) { }", "condition must be bool, got bit<8>"),
            (
                "if (false) { } else if (x) { }",
                "condition must be bool, got bit<8>",
            ),
            ("bit<8> y = x + 16w1;", "cannot add a bit<8> and a bit<16>"),
            (
                "bit<1> y = x[3];",
                "only slices supported as index arguments",
            ),
            (
                "bit<8> y = 8w1 << 129w1;",
                "Width 129 exceeds the 128-bit compiler limit",
            ),
            ("x = 16w1;", "cannot assign bit<16> to bit<8>"),
            ("x = true;", "cannot assign bool to bit<8>"),
            ("x = 8s1;", "cannot assign int<8> to bit<8>"),
            (
                "bool y = true < false;",
                "ordered comparison requires bit operands, got bool",
            ),
            (
                "bool y = 8s1 < 8s2;",
                "ordered comparison of int<8> is not supported by SoftNPU",
            ),
            (
                "bool y = 1 < 2;",
                "ordered comparison of a compile-time int is not supported \
                 by x4c",
            ),
            (
                "bit<8> y = 1 << 2;",
                "shifting a compile-time int is not supported by x4c",
            ),
            (
                "bool y = 1 < true;",
                "ordered comparison requires bit operands, got bool",
            ),
            (
                "bool y = 8s1 < true;",
                "ordered comparison requires bit operands, got bool",
            ),
            (
                "bit<8> y = 1 << true;",
                "shift count must be a bit type or a non-negative integer \
                 literal",
            ),
            ("bit<8> y = (1 == 2);", "cannot initialize bit<8> with bool"),
            (
                "bool y = (1 == 2) < false;",
                "ordered comparison requires bit operands, got bool",
            ),
            ("bit<8> y = 1 + 2;", super::UNSUPPORTED_INT_EXPRESSION),
            ("x = 1 + 2;", super::UNSUPPORTED_INT_EXPRESSION),
            ("x[7:0] = 1 + 2;", super::UNSUPPORTED_INT_EXPRESSION),
            (
                "int<8> s = 8s1; bit<4> y = s[3:0];",
                "slicing int<8> is not supported by SoftNPU",
            ),
            (
                "int<8> s = 8s1; s[3:0] = 4w1;",
                "slice assignment to int<8> is not supported by SoftNPU",
            ),
            (
                "int c; bit<4> y = c[3:0];",
                "slicing a compile-time int is not supported by x4c",
            ),
        ] {
            assert_eq!(
                errors_for(&statement_source(statement)),
                [message],
                "{statement}",
            );
        }
    }

    #[test]
    fn initializer_types_accepted() {
        for statement in [
            "bit<8> y = x;",
            "bit<16> y = x;",
            "bit<32> y = x;",
            "const bit<32> c = x;",
            "bit<8> y = 5;",
            "bool y = x == 8w1;",
            "if (x < 8w2) { }",
            "const bit<8> c = 5;",
            "x = 5;",
            "x = 8w2;",
        ] {
            assert_eq!(
                errors_for(&statement_source(statement)),
                Vec::<String>::new(),
                "{statement}",
            );
        }
    }

    fn shift_source(expression: &str) -> String {
        format!(
            r#"
control ingress() {{
    apply {{
        bit<32> x = 32w0x81234567;
        bit<8> narrow = 8w1;
        bit<128> wide = 128w1;
        int<8> signed_value;
        bool flag;
        bit<32> result = {expression};
    }}
}}
"#
        )
    }

    fn shift_expression(ast: &AST) -> &Expression {
        let Statement::Variable(result) =
            ast.controls[0].apply.statements.last().unwrap()
        else {
            panic!("expected result declaration");
        };
        result.initializer.as_deref().unwrap()
    }

    #[test]
    fn shift_count_types_preserve_result_type() {
        for op in ["<<", ">>"] {
            for count in
                ["0", "1", "31", "32", "33", "128", "256", "narrow", "wide"]
            {
                let source = shift_source(&format!("x {op} {count}"));
                let ast = parse_p4(&source);
                let (hlir, diags) = crate::check::all(&ast);
                let errors = diags.errors();
                assert!(
                    errors.is_empty(),
                    "{op} {count}: {:?}",
                    errors.iter().map(|d| &d.message).collect::<Vec<_>>(),
                );
                assert_eq!(
                    hlir.expression_types.get(shift_expression(&ast)),
                    Some(&Type::Bit(32)),
                    "{op} {count}",
                );
            }
        }
    }

    #[test]
    fn shift_operand_type_errors() {
        const COUNT_ERROR: &str =
            "shift count must be a bit type or a non-negative integer literal";

        for op in ["<<", ">>"] {
            for (lhs, rhs, message) in [
                (
                    "flag",
                    "1",
                    "shift left operand must be a bit type, got bool",
                ),
                (
                    "signed_value",
                    "1",
                    "shifting int<8> is not supported by SoftNPU",
                ),
                ("x", "flag", COUNT_ERROR),
                ("x", "signed_value", COUNT_ERROR),
            ] {
                let source = shift_source(&format!("{lhs} {op} {rhs}"));
                let ast = parse_p4(&source);
                let ExpressionKind::Binary(left, _, right) =
                    &shift_expression(&ast).kind
                else {
                    panic!("expected shift expression");
                };
                let token = match lhs {
                    "x" => &right.token,
                    _ => &left.token,
                };
                let (_, diags) = crate::check::all(&ast);
                let errors = diags.errors();
                assert_eq!(
                    errors.len(),
                    1,
                    "{lhs} {op} {rhs}: {:?}",
                    errors.iter().map(|d| &d.message).collect::<Vec<_>>(),
                );
                assert_eq!(errors[0].message, message);
                assert_eq!(
                    (errors[0].token.line, errors[0].token.col),
                    (token.line, token.col),
                );
            }
        }
    }

    #[test]
    fn negative_shift_count_rejected() {
        for op in ["<<", ">>"] {
            let source = shift_source(&format!("x {op} 1"));
            let mut ast = parse_p4(&source);
            let Statement::Variable(result) =
                ast.controls[0].apply.statements.last_mut().unwrap()
            else {
                panic!("expected result declaration");
            };
            let ExpressionKind::Binary(_, _, rhs) =
                &mut result.initializer.as_mut().unwrap().kind
            else {
                panic!("expected shift expression");
            };
            rhs.kind = ExpressionKind::IntegerLit(-1);
            let token = rhs.token.clone();
            let (_, diags) = crate::check::all(&ast);
            let errors = diags.errors();
            assert_eq!(errors.len(), 1);
            assert_eq!(
                errors[0].message,
                "shift count must be a bit type or a non-negative integer literal",
            );
            assert_eq!(
                (errors[0].token.line, errors[0].token.col),
                (token.line, token.col),
            );
        }
    }
}
