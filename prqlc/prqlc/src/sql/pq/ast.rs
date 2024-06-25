//! Sql Relational Query AST
//!
//! This IR dictates the structure of the resulting SQL query. This includes number of CTEs,
//! position of sub-queries and set operations.

use enum_as_inner::EnumAsInner;
use itertools::Itertools;
use prqlc_parser::generic::InterpolateItem;
use serde::Serialize;

use super::context::RIId;
use crate::ir::generic::ColumnSort;
use crate::ir::pl::JoinSide;
use crate::ir::rq::{self, fold_column_sorts, RelationLiteral, RqFold};
use crate::Result;

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
    SString(Vec<InterpolateItem<rq::Expr>>),
    Operator { name: String, args: Vec<rq::Expr> },
}

#[derive(Debug, Clone, Serialize)]
pub struct RelationExpr {
    pub kind: RelationExprKind,

    pub riid: RIId,
}

#[derive(Debug, Clone, Serialize)]
pub enum RelationExprKind {
    Ref(rq::TId),
    SubQuery(SqlRelation),
}

#[derive(Debug, Clone, Serialize)]
pub struct Cte {
    pub tid: rq::TId,
    pub kind: CteKind,
}

#[derive(Debug, Clone, Serialize)]
pub enum CteKind {
    Normal(SqlRelation),
    Loop {
        initial: SqlRelation,
        step: SqlRelation,
    },
}

/// Similar to [rq::Transform], but closer to a SQL clause.
///
/// Uses two generic args that allows the compiler to work in multiple stages:
/// - the first converts RQ to [SqlTransform<RIId, rq::Transform>],
/// - the second compiles that to [SqlTransform<RelationExpr, ()>].
#[derive(Debug, Clone, EnumAsInner, strum::AsRefStr, Serialize)]
pub enum SqlTransform<Rel = RIId, Super = rq::Transform> {
    /// Contains [rq::Transform] during compilation. After finishing, this is emptied.
    ///
    /// For example, initial an RQ Append transform is wrapped as such:
    ///
    /// ```ignore
    /// rq::Transform::Append(x) -> pq::SqlTransform::Super(rq::Transform::Append(x))
    /// ```
    ///
    /// During preprocessing it is compiled to:
    /// ```ignore
    /// pq::SqlTransform::Super(rq::Transform::Append(_)) -> pq::SqlTransform::Union { .. }
    /// ```
    ///
    /// At the end of PQ compilation, all `Super()` are either discarded or converted to their
    /// PQ equivalents.
    Super(Super),

    From(Rel),
    Select(Vec<rq::CId>),
    Filter(rq::Expr),
    Aggregate {
        partition: Vec<rq::CId>,
        compute: Vec<rq::CId>,
    },
    Sort(Vec<ColumnSort<rq::CId>>),
    Take(rq::Take),
    Join {
        side: JoinSide,
        with: Rel,
        filter: rq::Expr,
    },

    Distinct,
    DistinctOn(Vec<rq::CId>),
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

pub trait PqMapper<RelIn, RelOut, SuperIn, SuperOut>: RqFold {
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
    F: ?Sized + PqMapper<RelIn, RelOut, SuperIn, SuperOut>,
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
        SqlTransform::DistinctOn(ids) => SqlTransform::DistinctOn(fold.fold_cids(ids)?),
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
        SqlTransform::Select(v) => SqlTransform::Select(fold.fold_cids(v)?),
        SqlTransform::Filter(v) => SqlTransform::Filter(fold.fold_expr(v)?),
        SqlTransform::Aggregate { partition, compute } => SqlTransform::Aggregate {
            partition: fold.fold_cids(partition)?,
            compute: fold.fold_cids(compute)?,
        },
        SqlTransform::Sort(v) => SqlTransform::Sort(fold_column_sorts(fold, v)?),
        SqlTransform::Take(take) => SqlTransform::Take(rq::Take {
            partition: fold.fold_cids(take.partition)?,
            sort: fold_column_sorts(fold, take.sort)?,
            range: take.range,
        }),
    })
}

pub trait PqFold: PqMapper<RelationExpr, RelationExpr, (), ()> {
    fn fold_sql_query(&mut self, query: SqlQuery) -> Result<SqlQuery> {
        Ok(SqlQuery {
            ctes: query
                .ctes
                .into_iter()
                .map(|c| self.fold_cte(c))
                .try_collect()?,
            main_relation: self.fold_sql_relation(query.main_relation)?,
        })
    }

    fn fold_sql_relation(&mut self, relation: SqlRelation) -> Result<SqlRelation> {
        Ok(match relation {
            SqlRelation::AtomicPipeline(pipeline) => {
                SqlRelation::AtomicPipeline(self.fold_sql_transforms(pipeline)?)
            }
            _ => relation,
        })
    }

    fn fold_cte(&mut self, cte: Cte) -> Result<Cte> {
        Ok(Cte {
            tid: cte.tid,
            kind: match cte.kind {
                CteKind::Normal(rel) => CteKind::Normal(self.fold_sql_relation(rel)?),
                CteKind::Loop { initial, step } => CteKind::Loop {
                    initial: self.fold_sql_relation(initial)?,
                    step: self.fold_sql_relation(step)?,
                },
            },
        })
    }
}
