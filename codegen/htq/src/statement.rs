// Copyright 2024 Oxide Computer Company

use std::collections::HashMap;

use htq::ast::{Fset, Register, Rset, Value};
use p4::{
    ast::{
        DeclarationInfo, Direction, Expression, ExpressionKind, Lvalue,
        NameInfo, UserDefinedType,
    },
    hlir::Hlir,
};

use crate::{
    error::CodegenError, expression::emit_expression, AsyncFlagAllocator,
    CgContext, RegisterAllocator,
};

pub(crate) fn emit_statement(
    stmt: &p4::ast::Statement,
    ast: &p4::ast::AST,
    context: CgContext<'_>,
    hlir: &Hlir,
    names: &mut HashMap<String, NameInfo>,
    ra: &mut RegisterAllocator,
    afa: &mut AsyncFlagAllocator,
) -> Result<Vec<htq::ast::Statement>, CodegenError> {
    use p4::ast::Statement as S;
    match stmt {
        S::Empty => Ok(Vec::new()),
        S::Assignment(lval, expr) => {
            emit_assignment(hlir, ast, context, names, lval, expr, ra, afa)
        }
        S::Call(_call) => Ok(Vec::default()), //TODO
        S::If(_if_block) => Ok(Vec::default()), //TODO,
        S::Variable(_v) => Ok(Vec::default()), //TODO
        S::Constant(_c) => Ok(Vec::default()), //TODO
        S::Transition(_t) => Ok(Vec::default()), //TODO
        S::Return(_r) => Ok(Vec::default()),  //TODO
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_assignment(
    hlir: &Hlir,
    ast: &p4::ast::AST,
    context: CgContext<'_>,
    names: &mut HashMap<String, NameInfo>,
    target: &p4::ast::Lvalue,
    source: &p4::ast::Expression,
    ra: &mut RegisterAllocator,
    afa: &mut AsyncFlagAllocator,
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
            let _treg = Register::new(&target.name);
            // TODO store instr
        }
        DeclarationInfo::StructMember | DeclarationInfo::HeaderMember => {
            let treg = Register::new(target.root());
            let offsets = member_offsets(ast, names, target)?;
            let instr = Fset {
                offsets,
                typ: expr_value.typ,
                target: treg,
                source: Value::Register(expr_value.registers[0].clone()),
            };
            instrs.push(htq::ast::Statement::Fset(instr));
        }
        DeclarationInfo::Local => {
            let ty = hlir
                .expression_types
                .get(source)
                .ok_or(CodegenError::UntypedExpression(source.clone()))?;

            //TODO(ry) it's unfortunate that a code generate manually has to
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
                typ: expr_value.typ,
                source: Value::Register(expr_value.registers[0].clone()),
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