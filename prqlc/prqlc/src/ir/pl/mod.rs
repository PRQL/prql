//! Pipelined Language AST
//!
//! Abstract Syntax Tree for the first part of PRQL compiler.
//! It can represent basic expressions, lists, pipelines, function calls &
//! definitions, variable declarations and more.
//!
//! The central struct here is [Expr] and its [ExprKind].
//!
//! Top-level construct is a list of statements [`Vec<Stmt>`].

mod expr;
mod extra;
mod fold;
mod lineage;
mod stmt;
mod utils;

pub use self::expr::*;
pub use self::extra::*;
pub use self::fold::*;
pub use self::lineage::*;
pub use self::stmt::*;
pub use self::utils::*;
pub use crate::ast::{BinOp, BinaryExpr, Ident, Literal, UnOp, UnaryExpr, ValueAndUnit};

pub fn print_mem_sizes() {
    use crate::ast::{PrimitiveSet, Ty, TyFunc, TyKind, TyTupleField};
    use crate::ir::{decl, generic, pl, rq};
    use crate::sql::internal::SqlTransform;
    use crate::{ErrorMessage, ErrorMessages, SourceTree, Span};

    use std::mem::size_of;

    println!("{:16}= {}", "Annotation", size_of::<Annotation>());
    println!("{:16}= {}", "BinaryExpr", size_of::<BinaryExpr>());
    println!("{:16}= {}", "BinOp", size_of::<BinOp>());
    println!("{:16}= {}", "ColumnSort", size_of::<ColumnSort>());
    println!("{:16}= {}", "decl::Decl", size_of::<decl::Decl>());
    println!("{:16}= {}", "decl::DeclKind", size_of::<decl::DeclKind>());
    println!("{:16}= {}", "decl::Module", size_of::<decl::Module>());
    println!("{:16}= {}", "decl::TableDecl", size_of::<decl::TableDecl>());
    println!("{:16}= {}", "decl::TableExpr", size_of::<decl::TableExpr>());
    println!("{:16}= {}", "ErrorMessage", size_of::<ErrorMessage>());
    println!("{:16}= {}", "ErrorMessages", size_of::<ErrorMessages>());
    println!("{:16}= {}", "ExprKind", size_of::<ExprKind>());
    println!("{:16}= {}", "Func", size_of::<Func>());
    println!("{:16}= {}", "FuncCall", size_of::<FuncCall>());
    println!("{:16}= {}", "FuncParam", size_of::<FuncParam>());
    println!(
        "{:16}= {}",
        "generic::SortDirection",
        size_of::<generic::SortDirection>()
    );
    println!(
        "{:16}= {}",
        "generic::WindowKind",
        size_of::<generic::WindowKind>()
    );
    println!("{:16}= {}", "InterpolateItem", size_of::<InterpolateItem>());
    println!("{:16}= {}", "JoinSide", size_of::<JoinSide>());
    println!("{:16}= {}", "Lineage", size_of::<Lineage>());
    println!("{:16}= {}", "LineageColumn", size_of::<LineageColumn>());
    println!("{:16}= {}", "LineageInput", size_of::<LineageInput>());
    println!("{:16}= {}", "ModuleDef", size_of::<ModuleDef>());
    println!("{:16}= {}", "pl::Expr", size_of::<pl::Expr>());
    println!("{:16}= {}", "PrimitiveSet", size_of::<PrimitiveSet>());
    println!("{:16}= {}", "QueryDef", size_of::<QueryDef>());
    println!("{:16}= {}", "Range", size_of::<Range>());
    println!("{:16}= {}", "rq::Expr", size_of::<rq::Expr>());
    println!(
        "{:16}= {}",
        "rq::RelationalQuery",
        size_of::<rq::RelationalQuery>()
    );
    println!("{:16}= {}", "rq::TableRef", size_of::<rq::TableRef>());
    println!("{:16}= {}", "SourceTree", size_of::<SourceTree>());
    println!("{:16}= {}", "Span", size_of::<Span>());
    println!("{:16}= {}", "SqlTransform", size_of::<SqlTransform>());
    println!("{:16}= {}", "Stmt", size_of::<Stmt>());
    println!("{:16}= {}", "StmtKind", size_of::<StmtKind>());
    println!("{:16}= {}", "SwitchCase", size_of::<SwitchCase>());
    println!("{:16}= {}", "TableExternRef", size_of::<TableExternRef>());
    println!("{:16}= {}", "TransformCall", size_of::<TransformCall>());
    println!("{:16}= {}", "TransformKind", size_of::<TransformKind>());
    println!("{:16}= {}", "TupleField", size_of::<TyTupleField>());
    println!("{:16}= {}", "Ty", size_of::<Ty>());
    println!("{:16}= {}", "TyFunc", size_of::<TyFunc>());
    println!("{:16}= {}", "TyKind", size_of::<TyKind>());
    println!("{:16}= {}", "TyOrExpr", size_of::<TyOrExpr>());
    println!("{:16}= {}", "TypeDef", size_of::<TypeDef>());
    println!("{:16}= {}", "UnaryExpr", size_of::<UnaryExpr>());
    println!("{:16}= {}", "UnOp", size_of::<UnOp>());
    println!("{:16}= {}", "VarDef", size_of::<VarDef>());
    println!("{:16}= {}", "WindowFrame", size_of::<WindowFrame>());
}
