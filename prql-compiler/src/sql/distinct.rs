use anyhow::Result;

use crate::{
    ast::{FuncDef, Transform},
    internals::AstFold,
};

/// Creates `DISTINCT`s from `take`s
struct DistinctMaker {}

impl AstFold for DistinctMaker {
    fn fold_transform(&mut self, transform: Transform) -> Result<Transform> {
        Ok(match transform {
            Transform::Take { by, range } => {
                if by.is_empty() {
                    Transform::Take { range, by }
                } else {

                    Transform::Select(by)

                    // TODO: DISTINCT conditions
                    //  if range == 1..1 {
                    //    if by == all columns frame {
                    //      return `select distinct`
                    //    }
                    //  }
                    //  return `
                    //    derive _rn = row number over (order by)
                    //    filter (_rn | in range)
                    //  `
                }
            },
            _ => transform,
        })
    }

    fn fold_func_def(&mut self, function: FuncDef) -> Result<FuncDef> {
        Ok(function)
    }
}
