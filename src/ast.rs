#[derive(Debug)]
pub struct AST {
    pub constants: Vec<Constant>,
    pub headers: Vec<Header>,
    pub typedefs: Vec<Typedef>,
}

impl Default for AST {
    fn default() -> Self {
        Self{
            constants: Vec::new(),
            headers: Vec::new(),
            typedefs: Vec::new(),
        }
    }
}

/*
#[derive(Debug)]
pub enum Constant {
    Int(String, i128),
    Bit(String, u16, i128),
}
*/

#[derive(Debug, Clone)]
pub enum Type {
    Bool,
    Error,
    Bit(usize),
    Int(usize),
    String,
    UserDefined(String),
}

#[derive(Debug, Clone)]
pub struct Typedef {
    pub ty: Type,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct Constant {
    pub ty: Type,
    pub name: String,
    //TODO initializer: Expression,
}

#[derive(Debug, Clone)]
pub enum Expression {
    IntegerLit(i128),
    BitLit(i128),
}

#[derive(Debug, Clone)]
pub struct Header {
    pub name: String,
    pub members: Vec::<HeaderMember>,
}

impl Header {
    pub fn new(name: String) -> Self {
        Header{name,  members: Vec::new() }
    }
}

#[derive(Debug, Clone)]
pub struct HeaderMember {
    pub ty: Type,
    pub name: String,
}
