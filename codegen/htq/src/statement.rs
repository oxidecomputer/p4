// Copyright 2024 Oxide Computer Company

use std::collections::HashMap;

use htq::ast::{Beq, Fset, Register, Rset, StatementBlock, Value};
use p4::{
    ast::{
        BinOp, ControlParameter, DeclarationInfo, Direction, Expression,
        ExpressionKind, Lvalue, NameInfo, UserDefinedType,
    },
    hlir::Hlir,
};

use crate::{
    error::CodegenError,
    expression::{emit_expression, emit_single_valued_expression},
    AsyncFlagAllocator, P4Context, RegisterAllocator,
};

pub(crate) fn emit_statement(
    stmt: &p4::ast::Statement,
    ast: &p4::ast::AST,
    context: &P4Context<'_>,
    hlir: &Hlir,
    names: &mut HashMap<String, NameInfo>,
    ra: &mut RegisterAllocator,
    afa: &mut AsyncFlagAllocator,
    psub: &mut HashMap<ControlParameter, Vec<Register>>,
) -> Result<
    (Vec<htq::ast::Statement>, Vec<htq::ast::StatementBlock>),
    CodegenError,
> {
    use p4::ast::Statement as S;
    match stmt {
        S::Empty => Ok((Vec::default(), Vec::default())),
        S::Assignment(lval, expr) => {
            let stmts = emit_assignment(
                hlir, ast, context, names, lval, expr, ra, afa, psub,
            )?;
            Ok((stmts, Vec::default()))
        }
        S::Call(call) => {
            let stmts =
                emit_call(hlir, ast, context, names, call, ra, afa, psub)?;
            Ok((stmts, Vec::default()))
        }
        S::If(if_block) => {
            emit_if_block(hlir, ast, context, names, if_block, ra, afa, psub)
        }
        S::Variable(v) => {
            let stmts =
                emit_variable(hlir, ast, context, names, v, ra, afa, psub)?;
            Ok((stmts, Vec::default()))
        }
        S::Constant(_c) => Ok((Vec::default(), Vec::default())), //TODO
        S::Transition(_t) => Ok((Vec::default(), Vec::default())), //TODO
        S::Return(_r) => Ok((Vec::default(), Vec::default())),   //TODO
    }
}

fn emit_if_block(
    hlir: &Hlir,
    ast: &p4::ast::AST,
    context: &P4Context<'_>,
    names: &mut HashMap<String, NameInfo>,
    iblk: &p4::ast::IfBlock,
    ra: &mut RegisterAllocator,
    afa: &mut AsyncFlagAllocator,
    psub: &mut HashMap<ControlParameter, Vec<Register>>,
) -> Result<
    (Vec<htq::ast::Statement>, Vec<htq::ast::StatementBlock>),
    CodegenError,
> {
    let mut result = Vec::default();
    let mut blocks = Vec::default();

    let (source, predicate) =
        if let ExpressionKind::Binary(lhs, BinOp::Eq, rhs) =
            &iblk.predicate.kind
        {
            let (source_statements, source) = emit_single_valued_expression(
                lhs.as_ref(),
                hlir,
                ast,
                context,
                ra,
                afa,
                names,
            )?;
            result.extend(source_statements.clone());

            let (predicate_statements, predicate) =
                emit_single_valued_expression(
                    rhs.as_ref(),
                    hlir,
                    ast,
                    context,
                    ra,
                    afa,
                    names,
                )?;
            result.extend(predicate_statements.clone());

            (source, Value::reg(predicate))
        } else {
            let (predicate_statements, source) = emit_single_valued_expression(
                iblk.predicate.as_ref(),
                hlir,
                ast,
                context,
                ra,
                afa,
                names,
            )?;
            result.extend(predicate_statements.clone());
            (source, htq::ast::Value::bool(true))
        };

    let params = ra.all_registers();
    let args = params.clone().into_iter().map(Value::reg).collect();
    let label = format!("{}_hit", source.0);
    result.push(htq::ast::Statement::Beq(Beq {
        source,
        predicate,
        label: label.clone(),
        args,
    }));

    let mut blk = StatementBlock {
        name: label,
        parameters: params,
        statements: Vec::default(),
    };

    for stmt in &iblk.block.statements {
        // TODO nested if blocks
        let (stmts, _) =
            emit_statement(stmt, ast, context, hlir, names, ra, afa, psub)?;
        blk.statements.extend(stmts);
    }

    // XXX
    //result.push(htq::ast::Statement::Label(label, params));

    blocks.push(blk);

    Ok((result, blocks))
}

fn emit_variable(
    _hlir: &Hlir,
    _ast: &p4::ast::AST,
    _context: &P4Context<'_>,
    names: &mut HashMap<String, NameInfo>,
    var: &p4::ast::Variable,
    ra: &mut RegisterAllocator,
    _afa: &mut AsyncFlagAllocator,
    _psub: &mut HashMap<ControlParameter, Vec<Register>>,
) -> Result<Vec<htq::ast::Statement>, CodegenError> {
    //TODO(ry) it's unfortunate that a codegen module has to
    // manually maintain scope information. Perhaps we should be using
    // the AST visitor ... although I'm not sure if the AST visitor
    // maintains a scope either, if not it probably should ....
    names.insert(
        var.name.clone(),
        NameInfo {
            ty: var.ty.clone(),
            decl: DeclarationInfo::Local,
        },
    );

    if let Some(init) = &var.initializer {
        if let ExpressionKind::Lvalue(lval) = &init.kind {
            // TODO this could be modeled better (more explicitly) in the
            // AST
            if lval.leaf() == "await" {
                return Ok(vec![htq::ast::Statement::Await(htq::ast::Await {
                    source: Value::reg(
                        // TODO this is extermely fragile relying on a
                        // register naming convention.
                        ra.get(&format!("{}_sync", lval.root())).unwrap(),
                    ),
                })]);
            }
        }
    }

    Ok(Vec::default())
}

fn emit_call(
    hlir: &Hlir,
    ast: &p4::ast::AST,
    context: &P4Context<'_>,
    names: &mut HashMap<String, NameInfo>,
    call: &p4::ast::Call,
    ra: &mut RegisterAllocator,
    afa: &mut AsyncFlagAllocator,
    _psub: &mut HashMap<ControlParameter, Vec<Register>>,
) -> Result<Vec<htq::ast::Statement>, CodegenError> {
    let (instrs, _result) = match &context {
        P4Context::Control(c) => {
            crate::expression::emit_call(call, c, hlir, ast, ra, afa, names)?
        }
        P4Context::Parser(_) => {
            //TODO
            (Vec::default(), None)
        }
    };
    Ok(instrs)
}

#[allow(clippy::too_many_arguments)]
fn emit_assignment(
    hlir: &Hlir,
    ast: &p4::ast::AST,
    context: &P4Context<'_>,
    names: &mut HashMap<String, NameInfo>,
    target: &p4::ast::Lvalue,
    source: &p4::ast::Expression,
    ra: &mut RegisterAllocator,
    afa: &mut AsyncFlagAllocator,
    psub: &mut HashMap<ControlParameter, Vec<Register>>,
) -> Result<Vec<htq::ast::Statement>, CodegenError> {
    // Things that can be assigned to are lvalues with the following
    // declaration kinds. The table shows how each kind is referenced
    // in htq code.
    //
    // *==========================================*
    // | Decl Kind     | Htq Kind    | Instrs     |
    // |---------------+-------------+------------|
    // | Parameter     | Pointer     | load/store |
    // | Struct member | Pointer     | fget/fset  |
    // | Header member | Pointer     | fget/fset  |
    // | Local         | Register(s) | arithmetic |
    // *==========================================*
    //
    // Things that can be assigned from are expressions which have the
    // following kinds.
    //
    // Concrete types
    //
    // *=================================*
    // | Expr Kind            | Htq Kind |
    // |----------------------+----------|
    // | Bool literal         | bool     |
    // | Integer literal      | i128     |
    // | Bitfield Literal (N) | uN       |
    // | Signed Literal (N)   | iN       |
    // *=================================*
    //
    // Types that need to be resolved that eventually decay to a concrete
    // type.
    //
    // - lvalue
    // - binary
    // - index
    // - slice
    // - call
    // - list
    //

    let target_info = hlir
        .lvalue_decls
        .get(target)
        .ok_or(CodegenError::UndefinedLvalue(target.clone()))?;

    // reg typ
    let (mut instrs, expr_value) =
        emit_expression(source, hlir, ast, context, ra, afa, names)?;

    let expr_value = expr_value.ok_or(
        CodegenError::AssignmentExpressionRequiresValue(source.clone()),
    )?;

    if expr_value.registers.is_empty() {
        return Err(CodegenError::AssignmentExpressionRequiresValue(
            source.clone(),
        ));
    }

    match &target_info.decl {
        DeclarationInfo::Parameter(Direction::Out)
        | DeclarationInfo::Parameter(Direction::InOut) => {
            if target_info.ty.is_lookup_result() {
                if let P4Context::Control(control) = &context {
                    if let Some(param) = control.get_parameter(target.root()) {
                        psub.insert(
                            param.clone(),
                            expr_value
                                .registers
                                .iter()
                                .map(|x| ra.alloc_next(&x.0))
                                .collect(),
                        );
                    }
                }
            }
            // TODO store instr
        }
        DeclarationInfo::StructMember | DeclarationInfo::HeaderMember => {
            let treg = ra.alloc(target.root());
            let output = ra.alloc(target.root());
            let offsets = member_offsets(ast, names, target)?;
            let instr = Fset {
                output,
                offsets,
                typ: expr_value.registers[0].1.clone(),
                target: treg,
                source: Value::Register(expr_value.registers[0].0.clone()),
            };
            instrs.push(htq::ast::Statement::Fset(instr));
        }
        DeclarationInfo::Local => {
            let ty = hlir
                .expression_types
                .get(source)
                .ok_or(CodegenError::UntypedExpression(source.clone()))?;

            //TODO(ry) it's unfortunate that a codegen module has to
            // manually maintain scope information. Perhaps we should be using
            // the AST visitor ... although I'm not sure if the AST visitor
            // maintains a scope either, if not it probably should ....
            names.insert(
                target.name.clone(),
                NameInfo {
                    ty: ty.clone(),
                    decl: DeclarationInfo::Local,
                },
            );
            let target = ra.alloc(&target.name);

            let instr = Rset {
                target,
                typ: expr_value.registers[0].1.clone(),
                source: Value::Register(expr_value.registers[0].0.clone()),
            };
            instrs.push(htq::ast::Statement::Rset(instr));
        }
        _ => {
            return Err(CodegenError::InvalidAssignment(
                target_info.decl.clone(),
            ))
        }
    };

    Ok(instrs)
}

pub(crate) fn member_offsets(
    ast: &p4::ast::AST,
    names: &HashMap<String, NameInfo>,
    lval: &Lvalue,
) -> Result<Vec<Value>, CodegenError> {
    let root = lval.root();
    let info = names.get(root).ok_or(CodegenError::MemberParentNotFound(
        root.to_owned(),
        lval.clone(),
    ))?;
    let mut offsets = Vec::default();
    member_offsets_rec(ast, &info.ty, &mut offsets, lval.clone())?;
    Ok(offsets)
}

fn member_offsets_rec(
    ast: &p4::ast::AST,
    ty: &p4::ast::Type,
    offsets: &mut Vec<Value>,
    mut lval: Lvalue,
) -> Result<(), CodegenError> {
    match ty {
        p4::ast::Type::UserDefined(name, _) => {
            let root_type = ast.get_user_defined_type(name).ok_or(
                CodegenError::UserDefinedTypeNotFound(
                    name.clone(),
                    lval.token.clone(),
                ),
            )?;
            lval = lval.pop_left();
            let member = lval.root();
            //TODO
            let (offset, member_ty) = match &root_type {
                UserDefinedType::Struct(s) => {
                    let off = s.index_of(member).ok_or(
                        CodegenError::MemberOffsetNotFound(lval.clone()),
                    )?;
                    let member_info =
                        s.members.iter().find(|x| x.name == member).ok_or(
                            CodegenError::MemberNotFound(
                                member.to_owned(),
                                lval.clone(),
                            ),
                        )?;
                    (off, member_info.ty.clone())
                }
                UserDefinedType::Header(h) => {
                    let off = h.index_of(member).ok_or(
                        CodegenError::MemberOffsetNotFound(lval.clone()),
                    )?;
                    let member_info =
                        h.members.iter().find(|x| x.name == member).ok_or(
                            CodegenError::MemberNotFound(
                                member.to_owned(),
                                lval.clone(),
                            ),
                        )?;
                    (off, member_info.ty.clone())
                }
                UserDefinedType::Extern(e) => {
                    return Err(CodegenError::CannotOffsetExtern(
                        e.name.clone(),
                        lval.token.clone(),
                    ));
                }
            };
            let offset = i128::try_from(offset).map_err(|_| {
                CodegenError::NumericConversion(Expression {
                    token: lval.token.clone(),
                    kind: ExpressionKind::Lvalue(lval.clone()),
                })
            })?;
            offsets.push(Value::number(offset));
            if lval.degree() > 1 {
                member_offsets_rec(ast, &member_ty, offsets, lval.clone())?;
            }
        }
        _ => return Err(CodegenError::ExpectedStructParent(lval.clone())),
    }
    Ok(())
}
