//! Sql Relational Query AST
//!
//! This in an internal intermediate representation that wraps RQ nodes to extend possible node values.
//!
//! For example, RQ does not have a separate node for DISTINCT, but uses [crate::ast::rq::Take] 1 with
//! `partition`. In [super::preprocess] module, [crate::ast::rq::Transform] take is wrapped into
//! [SqlTransform], which does have [SqlTransform::Distinct].

use anyhow::Result;
use enum_as_inner::EnumAsInner;
use itertools::Itertools;
use serde::Serialize;

use crate::ast::rq::{Relation, RelationColumn, RelationKind, RqFold, TableRef, Transform};

#[derive(Debug, Clone, EnumAsInner)]
pub enum SqlRelationKind {
    Super(RelationKind),
    PreprocessedPipeline(Vec<SqlTransform>),
}

#[derive(Debug, Clone)]
pub struct SqlRelation {
    pub kind: SqlRelationKind,
    pub columns: Vec<RelationColumn>,
}

#[derive(Debug, Clone, EnumAsInner, strum::AsRefStr, Serialize)]
pub enum SqlTransform {
    Super(Transform),
    Distinct,
    Except { bottom: TableRef, distinct: bool },
    Intersect { bottom: TableRef, distinct: bool },
    Union { bottom: TableRef, distinct: bool },
    Loop(Vec<SqlTransform>),
}

impl SqlTransform {
    pub fn as_str(&self) -> &str {
        match self {
            SqlTransform::Super(t) => t.as_ref(),
            _ => self.as_ref(),
        }
    }

    pub fn into_super_and<T, F: FnOnce(Transform) -> Result<T, Transform>>(
        self,
        f: F,
    ) -> Result<T, SqlTransform> {
        self.into_super()
            .and_then(|t| f(t).map_err(SqlTransform::Super))
    }
}

impl From<Relation> for SqlRelation {
    fn from(rel: Relation) -> Self {
        SqlRelation {
            kind: SqlRelationKind::Super(rel.kind),
            columns: rel.columns,
        }
    }
}

pub trait SqlFold: RqFold {
    fn fold_sql_transforms(&mut self, transforms: Vec<SqlTransform>) -> Result<Vec<SqlTransform>> {
        transforms
            .into_iter()
            .map(|t| self.fold_sql_transform(t))
            .try_collect()
    }

    fn fold_sql_transform(&mut self, transform: SqlTransform) -> Result<SqlTransform> {
        Ok(match transform {
            SqlTransform::Super(t) => SqlTransform::Super(self.fold_transform(t)?),
            SqlTransform::Distinct => SqlTransform::Distinct,
            SqlTransform::Union { bottom, distinct } => SqlTransform::Union {
                bottom: self.fold_table_ref(bottom)?,
                distinct,
            },
            SqlTransform::Except { bottom, distinct } => SqlTransform::Except {
                bottom: self.fold_table_ref(bottom)?,
                distinct,
            },
            SqlTransform::Intersect { bottom, distinct } => SqlTransform::Intersect {
                bottom: self.fold_table_ref(bottom)?,
                distinct,
            },
            SqlTransform::Loop(pipeline) => SqlTransform::Loop(self.fold_sql_transforms(pipeline)?),
        })
    }
}
