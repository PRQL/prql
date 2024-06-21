use anyhow::Result;
use arrow::datatypes::{DataType, SchemaRef};
use connector_arrow::api::SchemaGet;
use itertools::Itertools;
use prqlc::pr::{PrimitiveSet, Stmt, Ty, TyKind, TyTupleField, VarDef};
use prqlc::{Error, WithErrorInfo};

use crate::ProjectCompiled;

#[cfg_attr(feature = "clap", derive(clap::Parser))]
pub struct PullSchemaParams {}

pub fn pull_schema(project: &ProjectCompiled, _params: PullSchemaParams) -> Result<Vec<Stmt>> {
    let project_root = project.sources.root.clone().unwrap();
    let db = &project.database_module;
    let mut conn = crate::connection::open(db, &project_root)?;

    let mut table_names = conn.table_list()?;
    table_names.sort();

    let mut defs: Vec<Stmt> = Vec::with_capacity(table_names.len());
    for table_name in table_names {
        let schema = conn.table_get(&table_name)?;

        defs.push(convert_arrow_schema_to_table_def(table_name, schema)?);
    }

    Ok(defs)
}

fn convert_arrow_schema_to_table_def(table_name: String, schema: SchemaRef) -> Result<Stmt> {
    let fields = schema
        .fields()
        .into_iter()
        .map(|field| -> Result<_> {
            let name = field.name();

            let ty = convert_arrow_type(field.data_type())
                .push_hint(format!("Found on table `{table_name}`, column `{name}`",))?;

            // TODO: handle field.is_nullable()

            Ok(TyTupleField::Single(Some(name.clone()), Some(Ty::new(ty))))
        })
        .try_collect()?;

    let def = VarDef {
        kind: prqlc::pr::VarDefKind::Let,
        name: table_name,
        value: None,
        ty: Some(Ty::new(TyKind::Array(Box::new(Ty::new(TyKind::Tuple(
            fields,
        )))))),
    };

    let stmt = Stmt::new(prqlc::pr::StmtKind::VarDef(def));

    Ok(stmt)
}

#[allow(clippy::result_large_err)]
fn convert_arrow_type(ty: &DataType) -> Result<TyKind, Error> {
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
            return Err(Error::new_simple(format!(
                "cannot convert arrow type {ty:?} a PRQL type"
            )))
        }
    })
}
