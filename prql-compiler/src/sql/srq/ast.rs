//! Sql Relational Query AST

use anyhow::Result;
use enum_as_inner::EnumAsInner;
use itertools::Itertools;
use serde::Serialize;

use crate::ast::pl::{ColumnSort, InterpolateItem, JoinSide, RelationLiteral};
use crate::ast::rq::{self, Expr, RqFold, TId};

#[derive(Debug, Clone)]
pub struct SqlQuery {
    /// Common Table Expression (WITH clause)
    pub ctes: Vec<(rq::TId, SqlRelation)>,

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

#[derive(Debug, Clone, EnumAsInner, strum::AsRefStr, Serialize)]
pub enum SqlTransform<Rel = RelationExpr, Super = rq::Transform> {
    // Contains [rq::Transform] during compilation. After finishing, this is emptied.
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
    Loop(Vec<SqlTransform>),
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
        SqlTransform::Loop(pipeline) => SqlTransform::Loop(pipeline),
        SqlTransform::Select(v) => SqlTransform::Select(v),
        SqlTransform::Filter(v) => SqlTransform::Filter(v),
        SqlTransform::Aggregate { partition, compute } => {
            SqlTransform::Aggregate { partition, compute }
        }
        SqlTransform::Sort(v) => SqlTransform::Sort(v),
        SqlTransform::Take(v) => SqlTransform::Take(v),
    })
}
