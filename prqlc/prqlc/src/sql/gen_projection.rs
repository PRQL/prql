use std::collections::{HashMap, HashSet};

use itertools::Itertools;
use sqlparser::ast::{
    self as sql_ast, ExceptSelectItem, ExcludeSelectItem, ObjectName, SelectItem,
    WildcardAdditionalOptions,
};

use super::dialect::ColumnExclude;
use super::gen_expr::*;
use super::pq::context::{AnchorContext, ColumnDecl};
use super::Context;
use crate::ir::pl::Ident;
use crate::ir::rq::{CId, RelationColumn};
use crate::Result;
use crate::{Error, Span, WithErrorInfo};

pub(super) fn try_into_exprs(
    cids: Vec<CId>,
    ctx: &mut Context,
    span: Option<Span>,
) -> Result<Vec<sql_ast::Expr>> {
    let (cids, excluded) = translate_wildcards(&ctx.anchor, cids);

    let mut res = Vec::new();
    for cid in cids {
        let decl = ctx.anchor.column_decls.get(&cid).unwrap();

        let ColumnDecl::RelationColumn(riid, _, RelationColumn::Wildcard) = decl else {
            // base case
            res.push(translate_cid(cid, ctx)?.into_ast());
            continue;
        };

        // star
        let t = &ctx.anchor.relation_instances[riid];
        let table_name = t.table_ref.name.clone().map(Ident::from_name);

        let ident = translate_star(ctx, span)?;
        if let Some(excluded) = excluded.get(&cid) {
            if !excluded.is_empty() {
                return Err(
                    Error::new_simple("Excluding columns not supported as this position")
                        .with_span(span),
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
    // This function adds that column to the exclusion tuple.
    fn exclude(star: &mut Option<(CId, HashSet<CId>)>, excluded: &mut Excluded) {
        let Some((cid, in_star)) = star.take() else {
            return;
        };
        if in_star.is_empty() {
            return;
        }

        excluded.insert(cid, in_star);
    }

    let mut output = Vec::new();
    for cid in cols {
        // don't use cols that have been included by preceding star
        let in_star = star
            .as_mut()
            .map(|s: &mut (CId, HashSet<CId>)| s.1.remove(&cid))
            .unwrap_or_default();
        if in_star {
            continue;
        }

        if let ColumnDecl::RelationColumn(riid, _, col) = &ctx.column_decls[&cid] {
            if matches!(col, RelationColumn::Wildcard) {
                exclude(&mut star, &mut excluded);

                let relation_instance = &ctx.relation_instances[riid];
                let mut in_star: HashSet<_> =
                    relation_instance.original_cids.iter().cloned().collect();
                in_star.remove(&cid);
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

        output.push(cid);
    }

    exclude(&mut star, &mut excluded);
    (output, excluded)
}

fn deduplicate_select_items(items: &mut Vec<SelectItem>) {
    // Dropping all duplicated identifiers
    let mut seen = HashSet::new();
    items.retain(|select_item| match select_item {
        SelectItem::UnnamedExpr(expr) => {
            if let sql_ast::Expr::CompoundIdentifier(idents) = expr {
                // If any of the identifiers hadn't been seen yet, retain the expr
                idents.iter().any(|ident| seen.insert(ident.clone()))
            } else {
                true
            }
        }
        SelectItem::ExprWithAlias { alias, .. } => seen.insert(alias.clone()),
        _ => true,
    });
}

pub(super) fn translate_select_items(
    cols: Vec<CId>,
    mut excluded: Excluded,
    ctx: &mut Context,
) -> Result<Vec<SelectItem>> {
    let mut res: Vec<_> = cols
        .into_iter()
        .map(|cid| {
            let decl = ctx.anchor.column_decls.get(&cid).unwrap();

            let ColumnDecl::RelationColumn(riid, _, RelationColumn::Wildcard) = decl else {
                // general case
                return translate_select_item(cid, ctx);
            };

            // wildcard case
            let t = &ctx.anchor.relation_instances[riid];
            let table_name = t.table_ref.name.clone().map(Ident::from_name);

            let ident = translate_ident(table_name, Some("*".to_string()), ctx);

            // excluded columns
            let opts = (excluded.remove(&cid))
                .and_then(|excluded| translate_exclude(ctx, excluded))
                .unwrap_or_default();

            Ok(if ident.len() > 1 {
                let mut object_name = ident;
                object_name.pop();
                SelectItem::QualifiedWildcard(
                    sqlparser::ast::SelectItemQualifiedWildcardKind::ObjectName(ObjectName(
                        object_name
                            .into_iter()
                            .map(sqlparser::ast::ObjectNamePart::Identifier)
                            .collect(),
                    )),
                    opts,
                )
            } else {
                SelectItem::Wildcard(opts)
            })
        })
        .try_collect()?;

    deduplicate_select_items(&mut res);

    if res.is_empty() && !ctx.dialect.supports_zero_columns() {
        // In some cases, no columns will appear in the projection
        // for SQL to parse correctly, we inject a `NULL`.
        // This is not strictly correct and should probably generate an error
        // instead.
        // Example: `from x | take 10 | aggregate { count this }`.
        // Here, first SELECT does not need to emit any columns as we don't need
        // any since we just count the number of rows.
        res.push(SelectItem::UnnamedExpr(sql_ast::Expr::Value(
            sql_ast::Value::Null.into(),
        )));
    }
    Ok(res)
}

fn translate_exclude(
    ctx: &mut Context,
    excluded: HashSet<CId>,
) -> Option<WildcardAdditionalOptions> {
    let excluded = as_col_names(&excluded, &ctx.anchor);

    let Some(supported) = ctx.dialect.column_exclude() else {
        // TODO: eventually this should throw an error
        //   I don't want to do this now, because we have no way around it.
        //   We could also ask the user to add table definitions.
        if log::log_enabled!(log::Level::Warn) {
            let excluded = excluded.join(", ");

            log::warn!("Columns {excluded} will be included with *, but were not requested.")
        }
        return None;
    };

    let mut excluded = excluded
        .into_iter()
        .map(|name| translate_ident_part(name.to_string(), ctx))
        .collect_vec();

    Some(match supported {
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
    })
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
