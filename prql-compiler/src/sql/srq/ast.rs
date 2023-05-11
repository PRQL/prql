//! Sql Relational Query AST
//!
//! This IR dictates the structure of the resulting SQL query. This includes number of CTEs,
//! position of sub-queries and set operations.

use anyhow::Result;
use enum_as_inner::EnumAsInner;
use itertools::Itertools;
use serde::Serialize;

use crate::ast::pl::{ColumnSort, InterpolateItem, JoinSide, RelationLiteral};
use crate::ast::rq::{self, fold_cids, fold_column_sorts, Expr, RqFold, TId, Take};

#[derive(Debug, Clone, Serialize)]
pub struct SqlQuery {
    /// Common Table Expression (WITH clause)
    pub ctes: Vec<Cte>,

    /// The body of SELECT query.
    pub main_relation: SqlRelation,
}

#[derive(Debug, Clone, EnumAsInner, Serialize)]
pub enum SqlRelation {
    AtomicPipeline(Vec<SqlTransform<RelationExpr, ()>>),
    Literal(RelationLiteral),
    SString(Vec<InterpolateItem<Expr>>),
}

#[derive(Debug, Clone, Serialize)]
pub enum RelationExpr {
    Ref(TId, Option<String>),
    SubQuery(SqlRelation, Option<String>),
}

#[derive(Debug, Clone, Serialize)]
pub struct Cte {
    pub tid: TId,
    pub kind: CteKind,
}

#[derive(Debug, Clone, Serialize)]
pub enum CteKind {
    Normal(SqlRelation),
    Loop {
        initial: SqlRelation,
        step: SqlRelation,
        recursive_name: String,
    },
}

/// Similar to [rq::Transform], but closer to a SQL clause.
///
/// Uses two generic args that allows the compiler to work in multiple stages.
/// First convert RQ to [SqlTransform<TableRef, rq::Transform>] and at the end
/// compile that to [SqlTransform<RelationExpr, ()>].
#[derive(Debug, Clone, EnumAsInner, strum::AsRefStr, Serialize)]
pub enum SqlTransform<Rel = RelationExpr, Super = rq::Transform> {
    /// Contains [rq::Transform] during compilation. After finishing, this is emptied.
    ///
    /// For example, initial an RQ Append transform is wrapped as such:
    ///
    /// ```ignore
    /// rq::Transform::Append(x) -> srq::SqlTransform::Super(rq::Transform::Append(x))
    /// ```
    ///
    /// During preprocessing, `Super(Append)` is converted into `srq::SqlTransform::Union { .. }`.
    ///
    /// At the end of SRQ compilation, all `Super()` are either discarded or converted to their
    /// SRQ equivalents.
    Super(Super),

    From(Rel),
    Select(Vec<rq::CId>),
    Filter(Expr),
    Aggregate {
        partition: Vec<rq::CId>,
        compute: Vec<rq::CId>,
    },
    Sort(Vec<ColumnSort<rq::CId>>),
    Take(rq::Take),
    Join {
        side: JoinSide,
        with: Rel,
        filter: Expr,
    },

    Distinct,
    Except {
        bottom: Rel,
        distinct: bool,
    },
    Intersect {
        bottom: Rel,
        distinct: bool,
    },
    Union {
        bottom: Rel,
        distinct: bool,
    },
}

impl<Rel> SqlTransform<Rel> {
    pub fn as_str(&self) -> &str {
        match self {
            SqlTransform::Super(t) => t.as_ref(),
            _ => self.as_ref(),
        }
    }

    pub fn into_super_and<T, F: FnOnce(rq::Transform) -> Result<T, rq::Transform>>(
        self,
        f: F,
    ) -> Result<T, Self> {
        self.into_super()
            .and_then(|t| f(t).map_err(SqlTransform::Super))
    }
}

pub trait SrqFold<RelIn, RelOut, SuperIn, SuperOut>: RqFold {
    fn fold_rel(&mut self, rel: RelIn) -> Result<RelOut>;

    fn fold_super(&mut self, sup: SuperIn) -> Result<SuperOut>;

    fn fold_sql_transforms(
        &mut self,
        transforms: Vec<SqlTransform<RelIn, SuperIn>>,
    ) -> Result<Vec<SqlTransform<RelOut, SuperOut>>> {
        transforms
            .into_iter()
            .map(|t| self.fold_sql_transform(t))
            .try_collect()
    }

    fn fold_sql_transform(
        &mut self,
        transform: SqlTransform<RelIn, SuperIn>,
    ) -> Result<SqlTransform<RelOut, SuperOut>> {
        fold_sql_transform::<RelIn, RelOut, SuperIn, SuperOut, _>(self, transform)
    }
}

pub fn fold_sql_transform<
    RelIn,
    RelOut,
    SuperIn,
    SuperOut,
    F: ?Sized + SrqFold<RelIn, RelOut, SuperIn, SuperOut>,
>(
    fold: &mut F,
    transform: SqlTransform<RelIn, SuperIn>,
) -> Result<SqlTransform<RelOut, SuperOut>> {
    Ok(match transform {
        SqlTransform::Super(t) => SqlTransform::Super(fold.fold_super(t)?),

        SqlTransform::From(rel) => SqlTransform::From(fold.fold_rel(rel)?),
        SqlTransform::Join { side, with, filter } => SqlTransform::Join {
            side,
            with: fold.fold_rel(with)?,
            filter: fold.fold_expr(filter)?,
        },

        SqlTransform::Distinct => SqlTransform::Distinct,
        SqlTransform::Union { bottom, distinct } => SqlTransform::Union {
            bottom: fold.fold_rel(bottom)?,
            distinct,
        },
        SqlTransform::Except { bottom, distinct } => SqlTransform::Except {
            bottom: fold.fold_rel(bottom)?,
            distinct,
        },
        SqlTransform::Intersect { bottom, distinct } => SqlTransform::Intersect {
            bottom: fold.fold_rel(bottom)?,
            distinct,
        },
        SqlTransform::Select(v) => SqlTransform::Select(fold_cids(fold, v)?),
        SqlTransform::Filter(v) => SqlTransform::Filter(fold.fold_expr(v)?),
        SqlTransform::Aggregate { partition, compute } => SqlTransform::Aggregate {
            partition: fold_cids(fold, partition)?,
            compute: fold_cids(fold, compute)?,
        },
        SqlTransform::Sort(v) => SqlTransform::Sort(fold_column_sorts(fold, v)?),
        SqlTransform::Take(take) => SqlTransform::Take(Take {
            partition: fold_cids(fold, take.partition)?,
            sort: fold_column_sorts(fold, take.sort)?,
            range: take.range,
        }),
    })
}
