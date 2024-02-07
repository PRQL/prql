use anyhow::Result;
use arrow::datatypes::DataType;
use connector_arrow::api::RelationDef;
use itertools::Itertools;
use prqlc::ast::{PrimitiveSet, Stmt, TupleField, Ty, TyKind, VarDef};
use prqlc::{Error, WithErrorInfo};

use crate::ProjectCompiled;

#[cfg_attr(feature = "clap", derive(clap::Parser))]
pub struct PullSchemaParams {}

pub fn pull_schema(project: &ProjectCompiled, _params: PullSchemaParams) -> Result<Vec<Stmt>> {
    let project_root = project.sources.root.clone().unwrap();
    let db = &project.database_module;
    let mut conn = crate::connection::open(db, &project_root)?;

    let mut defs = connector_arrow::api::Connection::get_relation_defs(&mut conn)?;

    defs.sort_by(|a, b| a.name.cmp(&b.name));

    let defs: Vec<Stmt> = defs
        .into_iter()
        .map(convert_arrow_schema_to_table_def)
        .try_collect()?;

    Ok(defs)
}

fn convert_arrow_schema_to_table_def(relation_def: RelationDef) -> Result<Stmt> {
    let fields = relation_def
        .schema
        .fields()
        .into_iter()
        .map(|field| -> Result<_> {
            let name = field.name();

            let res = convert_arrow_type(field.data_type());
            let ty = res.push_hint(format!(
                "Found on table `{}`, column `{}`",
                &relation_def.name, &name
            ))?;

            // TODO: handle field.is_nullable()

            Ok(TupleField::Single(Some(name.clone()), Some(Ty::new(ty))))
        })
        .try_collect()?;

    let def = VarDef {
        kind: prqlc::ast::VarDefKind::Let,
        name: relation_def.name,
        value: None,
        ty: Some(Ty::new(TyKind::Array(Box::new(Ty::new(TyKind::Tuple(
            fields,
        )))))),
    };

    let stmt = Stmt::new(prqlc::ast::StmtKind::VarDef(def));

    Ok(stmt)
}

fn convert_arrow_type(ty: &DataType) -> Result<TyKind> {
    Ok(match ty {
        DataType::Boolean => TyKind::Primitive(PrimitiveSet::Bool),
        DataType::Int8 => TyKind::Primitive(PrimitiveSet::Int),
        DataType::Int16 => TyKind::Primitive(PrimitiveSet::Int),
        DataType::Int32 => TyKind::Primitive(PrimitiveSet::Int),
        DataType::Int64 => TyKind::Primitive(PrimitiveSet::Int),
        DataType::UInt8 => TyKind::Primitive(PrimitiveSet::Int),
        DataType::UInt16 => TyKind::Primitive(PrimitiveSet::Int),
        DataType::UInt32 => TyKind::Primitive(PrimitiveSet::Int),
        DataType::UInt64 => TyKind::Primitive(PrimitiveSet::Int),
        DataType::Float16 => TyKind::Primitive(PrimitiveSet::Float),
        DataType::Float32 => TyKind::Primitive(PrimitiveSet::Float),
        DataType::Float64 => TyKind::Primitive(PrimitiveSet::Float),
        DataType::Timestamp(_, _) => TyKind::Primitive(PrimitiveSet::Timestamp),
        DataType::Date32 => TyKind::Primitive(PrimitiveSet::Date),
        DataType::Date64 => TyKind::Primitive(PrimitiveSet::Date),
        DataType::Time32(_) => TyKind::Primitive(PrimitiveSet::Time),
        DataType::Time64(_) => TyKind::Primitive(PrimitiveSet::Time),
        DataType::Utf8 => TyKind::Primitive(PrimitiveSet::Text),
        DataType::LargeUtf8 => TyKind::Primitive(PrimitiveSet::Text),
        DataType::Null
        | DataType::Duration(_)
        | DataType::Interval(_)
        | DataType::Binary
        | DataType::FixedSizeBinary(_)
        | DataType::LargeBinary
        | DataType::List(_)
        | DataType::FixedSizeList(_, _)
        | DataType::LargeList(_)
        | DataType::Struct(_)
        | DataType::Union(_, _)
        | DataType::Dictionary(_, _)
        | DataType::Decimal128(_, _)
        | DataType::Decimal256(_, _)
        | DataType::Map(_, _)
        | DataType::RunEndEncoded(_, _) => {
            return Err(
                Error::new_simple(format!("cannot convert arrow type {ty:?} a PRQL type")).into(),
            )
        }
    })
}
