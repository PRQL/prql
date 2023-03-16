use std::collections::HashMap;

use anyhow::Result;
use itertools::Itertools;
use regex::Regex;

use crate::{
    ast::{
        pl::InterpolateItem,
        rq::{self, Expr, ExprKind, RelationKind, RqFold},
    },
    utils::NameGenerator,
};

pub fn extract(query: rq::Query) -> (rq::Query, SStrings) {
    let mut e = SStringExtractor {
        id_gen: NameGenerator::new("s_string_"),
        strings: HashMap::new(),
    };

    let query = e.fold_query(query).unwrap();

    (query, SStrings(e.strings))
}

pub fn substitute(sql: String, s_strings: SStrings) -> String {
    let mut res = String::new();

    let re = Regex::new(&r"'(s_string_\d+)'|(SELECT(\n|\s)+)?(s_string_\d+)").unwrap();

    let mut last_index = 0;
    for cap in re.captures_iter(sql.as_str()) {
        // use last capture group
        let index = cap.get(0).unwrap().start();
        res += &sql[last_index..index];

        if let Some(match_) = cap.get(cap.len() - 1) {
            let id = match_.as_str();
            res += s_strings.0.get(id).map(|s| s.as_str()).unwrap_or(&id);
        }

        last_index = cap.get(0).unwrap().end();
    }
    res += &sql[last_index..];

    res
}

pub struct SStrings(HashMap<String, String>);

struct SStringExtractor {
    id_gen: NameGenerator,

    strings: HashMap<String, String>,
}

impl SStringExtractor {
    pub fn fold_interpolate_items(
        &mut self,
        items: Vec<InterpolateItem<Expr>>,
    ) -> Vec<InterpolateItem<Expr>> {
        items
            .into_iter()
            .map(|item| match item {
                InterpolateItem::String(value) => {
                    let id = self.id_gen.gen();
                    self.strings.insert(id.clone(), value);

                    InterpolateItem::String(format!("'{id}'"))
                }
                InterpolateItem::Expr(_) => item,
            })
            .collect_vec()
    }
}

impl RqFold for SStringExtractor {
    fn fold_relation_kind(&mut self, kind: RelationKind) -> Result<RelationKind> {
        match kind {
            RelationKind::SString(items) => {
                let mut items = self.fold_interpolate_items(items);

                // insert a "SELECT " item at the beginning, so the string can translate
                // to [sqlparser::ast::Select]
                let id = self.id_gen.gen();
                items.insert(0, InterpolateItem::String(format!("SELECT {id}")));
                self.strings.insert(id, "".to_string());

                Ok(RelationKind::SString(items))
            }
            kind => rq::fold_relation_kind(self, kind),
        }
    }

    fn fold_expr_kind(&mut self, kind: ExprKind) -> Result<ExprKind> {
        match kind {
            ExprKind::SString(items) => Ok(ExprKind::SString(self.fold_interpolate_items(items))),
            kind => rq::fold_expr_kind(self, kind),
        }
    }
}
