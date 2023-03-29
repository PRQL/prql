use std::collections::{HashMap, HashSet};

use anyhow::Result;
use itertools::Itertools;
use sqlparser::ast::{
    self as sql_ast, ExceptSelectItem, ExcludeSelectItem, ObjectName, SelectItem,
    WildcardAdditionalOptions,
};

use crate::ast::rq::{CId, RelationColumn};

use crate::error::{Error, Span};
use crate::sql::context::ColumnDecl;

use super::context::AnchorContext;
use super::dialect::ColumnExclude;
use super::{gen_expr::*, Context};

pub(super) fn try_into_exprs(
    cids: Vec<CId>,
    ctx: &mut Context,
    span: Option<Span>,
) -> Result<Vec<sql_ast::Expr>> {
    let (cids, excluded) = translate_wildcards(&ctx.anchor, cids);

    let mut res = Vec::new();
    for cid in cids {
        let decl = ctx.anchor.column_decls.get(&cid).unwrap();

        let ColumnDecl::RelationColumn(tiid, _, RelationColumn::Wildcard) = decl else {
            // base case
            res.push(translate_cid(cid, ctx)?);
            continue;
        };

        // star
        let t = &ctx.anchor.table_instances[tiid];
        let table_name = t.name.clone();

        let ident = translate_star(ctx, span)?;
        if let Some(excluded) = excluded.get(&cid) {
            if !excluded.is_empty() {
                return Err(
                    Error::new_simple("Excluding columns not supported as this position")
                        .with_span(span)
                        .into(),
                );
            }
        }
        let ident = translate_ident(table_name, Some(ident), ctx);

        res.push(sql_ast::Expr::CompoundIdentifier(ident));
    }
    Ok(res)
}

type Excluded = HashMap<CId, HashSet<CId>>;

/// Convert RQ wildcards to SQL stars.
/// Note that they don't have the same semantics:
/// - wildcard means "other columns that we don't have the knowledge of"
/// - star means "all columns of the table"
///
pub(super) fn translate_wildcards(ctx: &AnchorContext, cols: Vec<CId>) -> (Vec<CId>, Excluded) {
    let mut star = None;
    let mut excluded: Excluded = HashMap::new();

    // When compiling:
    // from employees | group department (take 3)
    // Row number will be computed in a CTE that also contains a star.
    // In the main query, star will also include row number, which was not
    // requested.
    // This function adds that column to the exclusion list.
    fn exclude(star: &mut Option<(CId, HashSet<CId>)>, excluded: &mut Excluded) {
        let Some((cid, in_star)) = star.take() else { return };
        if in_star.is_empty() {
            return;
        }

        excluded.insert(cid, in_star);
    }

    let mut output = Vec::new();
    for cid in cols {
        if let ColumnDecl::RelationColumn(tiid, _, col) = &ctx.column_decls[&cid] {
            if matches!(col, RelationColumn::Wildcard) {
                exclude(&mut star, &mut excluded);

                let table_ref = &ctx.table_instances[tiid];
                let in_star: HashSet<_> = (table_ref.columns)
                    .iter()
                    .filter_map(|c| match c {
                        (RelationColumn::Wildcard, _) => None,
                        (_, cid) => Some(*cid),
                    })
                    .collect();
                star = Some((cid, in_star));

                // remove preceding cols that will be included with this star
                if let Some((_, in_star)) = &mut star {
                    while let Some(prev) = output.pop() {
                        if !in_star.remove(&prev) {
                            output.push(prev);
                            break;
                        }
                    }
                }
            }
        }

        // don't use cols that have been included by preceding star
        let in_star = star.as_mut().map(|s| s.1.remove(&cid)).unwrap_or_default();
        if !in_star {
            output.push(cid);
        }
    }

    exclude(&mut star, &mut excluded);
    (output, excluded)
}

pub(super) fn translate_select_items(
    cols: Vec<CId>,
    mut excluded: Excluded,
    ctx: &mut Context,
) -> Result<Vec<SelectItem>> {
    cols.into_iter()
        .map(|cid| {
            let decl = ctx.anchor.column_decls.get(&cid).unwrap();

            let ColumnDecl::RelationColumn(tiid, _, RelationColumn::Wildcard) = decl else {
                // general case
                return translate_select_item(cid, ctx)
            };

            // wildcard case
            let t = &ctx.anchor.table_instances[tiid];
            let table_name = t.name.clone();

            let ident = translate_ident(table_name, Some("*".to_string()), ctx);

            // excluded columns
            let opts = (excluded.remove(&cid))
                .and_then(|x| translate_exclude(ctx, x).transpose())
                .unwrap_or_else(|| Ok(WildcardAdditionalOptions::default()))?;

            Ok(if ident.len() > 1 {
                let mut object_name = ident;
                object_name.pop();
                SelectItem::QualifiedWildcard(ObjectName(object_name), opts)
            } else {
                SelectItem::Wildcard(opts)
            })
        })
        .try_collect()
}

fn translate_exclude(
    ctx: &mut Context,
    excluded: HashSet<CId>,
) -> Result<Option<WildcardAdditionalOptions>> {
    let excluded = as_col_names(&excluded, &ctx.anchor);

    let Some(supported) = ctx.dialect.column_exclude() else {
        let excluded = excluded.join(", ");
        // TODO: can we specify `span` here?
        // TODO: can we get a nicer name for the dialect?
        let dialect = &ctx.dialect;
        return Err(Error::new_simple(format!("Excluding columns ({excluded}) is not supported by the current dialect, {dialect:?}")).with_help("Consider specifying the full set of columns prior with a `select`").into());
    };

    let mut excluded = excluded
        .into_iter()
        .map(|name| translate_ident_part(name.to_string(), ctx))
        .collect_vec();

    Ok(Some(match supported {
        ColumnExclude::Exclude => WildcardAdditionalOptions {
            opt_exclude: Some(ExcludeSelectItem::Multiple(excluded)),
            ..Default::default()
        },
        ColumnExclude::Except => WildcardAdditionalOptions {
            opt_except: Some(ExceptSelectItem {
                first_element: excluded.remove(0),
                additional_elements: excluded,
            }),
            ..Default::default()
        },
    }))
}

fn as_col_names<'a>(cids: &'a HashSet<CId>, ctx: &'a AnchorContext) -> Vec<&'a str> {
    cids.iter()
        .sorted_by_key(|c| c.get())
        .map(|c| {
            ctx.column_decls
                .get(c)
                .and_then(|c| match c {
                    ColumnDecl::RelationColumn(_, _, rc) => rc.as_single().map(|o| o.as_ref()),
                    _ => None,
                })
                .flatten()
                .map(|n| n.as_str())
                .unwrap_or("<unnamed>")
        })
        .collect_vec()
}
