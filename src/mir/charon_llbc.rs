//! Charon LLBC JSON representation
//!
//! Based on AeneasVerif/charon's exported JSON format for Low-Level Borrow Calculus (LLBC).

use serde::{Deserialize, Serialize};

/// Top-level structure for a Charon exported crate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Crate {
    pub crate_name: String,
    pub files: Vec<File>,
    pub fun_decls: Vec<FunDecl>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct File {
    pub path: String,
    pub contents: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunDecl {
    pub def_id: u32,
    pub name: Vec<String>,
    pub meta: Meta,
    pub signature: FunSignature,
    pub body: Option<Body>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meta {
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    pub file_id: u32,
    pub beg: Loc,
    pub end: Loc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Loc {
    pub line: u32,
    pub col: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunSignature {
    pub inputs: Vec<Ty>,
    pub output: Ty,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum Ty {
    Adt { id: String, generics: Generics },
    Bool,
    Char,
    Integer(IntegerTy),
    Str,
    Array(Box<Ty>, usize),
    Slice(Box<Ty>),
    Ref(Box<Ty>, RefKind),
    Never,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IntegerTy {
    Isize,
    I8,
    I16,
    I32,
    I64,
    I128,
    Usize,
    U8,
    U16,
    U32,
    U64,
    U128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RefKind {
    Mut,
    Shared,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Generics {
    pub types: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Body {
    pub arg_count: usize,
    pub locals: Vec<Local>,
    #[serde(rename = "unstructured_body")]
    pub unstructured: Option<UllbcBody>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Local {
    pub name: Option<String>,
    pub ty: Ty,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UllbcBody {
    pub blocks: Vec<BlockData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockData {
    pub id: u32,
    pub statements: Vec<Statement>,
    pub terminator: Terminator,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum Statement {
    Assign(Place, Rvalue),
    FakeRead(Place),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum Terminator {
    Goto(u32),
    #[serde(rename_all = "camelCase")]
    SwitchInt {
        discr: Operand,
        targets: Vec<SwitchTarget>,
        otherwise: u32,
    },
    Return,
    Panic,
    #[serde(rename_all = "camelCase")]
    Call {
        func: FnPtr,
        args: Vec<Operand>,
        dest: Place,
        yield_target: Option<u32>,
        target: Option<u32>,
    },
    #[serde(rename_all = "camelCase")]
    Assert {
        cond: Operand,
        expected: bool,
        target: u32,
    },
    #[serde(rename_all = "camelCase")]
    Drop {
        place: Place,
        target: u32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum FnPtr {
    Regular { id: u32, generics: Generics },
    // Add other variants if needed
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwitchTarget {
    pub value: String,
    pub target: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum Place {
    Local(u32),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum Rvalue {
    BinaryOp(BinOp, Operand, Operand),
    UnaryOp(UnOp, Operand),
    Use(Operand),
    Constant(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UnOp {
    Not,
    Neg,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum Operand {
    Copy(Place),
    Move(Place),
    Constant(String),
}
