use p4::ast::{AST, Direction, Type, Struct};
use p4::check::{Diagnostics, Diagnostic, Level};

pub fn emit(ast: &AST) -> Diagnostics {

    let mut diags = Vec::new();

    handle_parsers(ast, &mut diags);

    Diagnostics(diags)

}

fn handle_parsers(ast: &AST, diags: &mut Vec<Diagnostic>) {

    // - iterate through parsers and look at headers
    // - generate a Struct object for each struct
    // - generate a Header object for each header

    //
    // iterate through the parsers, looking for out parameters and generating
    // Struct and Header object for the ones we find.
    //
    for parser in &ast.parsers {
        for parameter in &parser.parameters {

            // ignore parameters not in an out direction, we're just generating
            // supporting data structures right now.
            if parameter.direction != Direction::Out {
                continue;
            }
            if let Type::UserDefined(ref typename) = parameter.ty {
                if let Some(decl) = ast.get_struct(typename) {
                    emit_struct(ast, decl, diags)
                }
                else {
                    // semantic error undefined type
                    diags.push(Diagnostic{
                        level: Level::Error,
                        message: format!(
                            "Undefined type {}",
                            parameter.ty,
                        ),
                        token: parameter.ty_token.clone(),
                    });
                }
            }
            else {
                // semantic error, out parameters must be structures
                diags.push(Diagnostic{
                    level: Level::Error,
                    message: format!(
                        "Out parameter must be a struct, found {}",
                        parameter.ty,
                    ),
                    token: parameter.ty_token.clone(),
                });
                    
            }
        }
    }

}

fn emit_struct(_ast: &AST, _s: &Struct, _diags: &mut Vec<Diagnostic>) {
}
