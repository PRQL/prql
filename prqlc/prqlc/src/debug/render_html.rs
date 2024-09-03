use std::collections::HashMap;
use std::fmt::{Debug, Result, Write};

use crate::sql::pq_ast;
use crate::{codegen, SourceTree};

use super::log::*;
use crate::ir::{decl, pl, rq};
use itertools::Itertools;
use prqlc_parser::lexer::lr;
use prqlc_parser::parser::pr;

pub fn render_log_to_html<W: std::io::Write>(writer: W, debug_log: &DebugLog) -> core::fmt::Result {
    struct IoWriter<W: std::io::Write> {
        inner: W,
    }

    impl<W: std::io::Write> core::fmt::Write for IoWriter<W> {
        fn write_str(&mut self, s: &str) -> std::fmt::Result {
            self.inner
                .write(s.as_bytes())
                .map_err(|_| std::fmt::Error)?;
            Ok(())
        }
    }
    let mut io_writer = IoWriter { inner: writer };

    write_debug_log(&mut io_writer, debug_log)
}

fn write_debug_log<W: Write>(w: &mut W, debug_log: &DebugLog) -> Result {
    writeln!(w, "<!doctype html>")?;
    writeln!(w, "<html>")?;
    writeln!(w, "<head>")?;
    writeln!(w, r#"<meta charset="utf-8">"#)?;
    writeln!(w, "<title>prqlc debug log {}</title>", debug_log.started_at)?;
    writeln!(w, r#"<meta name="generator" content="prqlc">"#)?;
    writeln!(w, r#"<meta name="robots" content="noindex">"#)?;
    writeln!(w, "<style>{}</style>", CSS_STYLES)?;
    writeln!(w, "<script>{}</script>", JS_SCRIPT)?;
    writeln!(w, "</head>")?;
    writeln!(w, "<body>")?;

    // header
    writeln!(w, "<header>")?;
    writeln!(w, "<h1>prqlc debug log</h1>")?;

    write_key_values(
        w,
        &[
            ("started_at", &debug_log.started_at),
            ("version", &debug_log.version),
        ],
    )?;

    writeln!(w, "</header>")?;
    writeln!(w, "<main>")?;

    for entry in debug_log.entries.iter() {
        writeln!(w, r#"<div class="entry">"#)?;

        match &entry.kind {
            DebugEntryKind::NewStage(stage) => {
                let substage = stage
                    .sub_stage()
                    .map(|s| s.to_lowercase())
                    .unwrap_or_default();
                let stage = stage.as_ref().to_lowercase();

                writeln!(w, r#"<h2 class="muted">{stage} {substage}</h2>"#)?;
            }
            DebugEntryKind::Message(message) => {
                write_message(w, message)?;
            }
            _ => {
                write_titled_entry(w, entry)?;
            }
        }

        writeln!(w, "</div>")?; // entry
    }

    writeln!(w, "</main>")?;
    writeln!(w, "</body>")?;
    writeln!(w, "</html>")?;

    Ok(())
}

fn write_titled_entry<W: Write>(w: &mut W, entry: &DebugEntry) -> Result {
    writeln!(w, "<details open>")?;
    writeln!(w, r#"<summary class="entry-label clickable">"#)?;
    let kind = entry.kind.as_ref()[4..].to_ascii_uppercase();
    writeln!(
        w,
        r#"[<b>REPRESENTATION</b>] <span class="yellow">{kind}</span>"#,
    )?;
    writeln!(w, "</summary>")?;
    writeln!(w, r#"<div class="entry-content">"#)?;
    match &entry.kind {
        DebugEntryKind::ReprPrql(a) => write_repr_prql(w, a)?,
        DebugEntryKind::ReprLr(a) => write_repr_lr(w, a)?,
        DebugEntryKind::ReprPr(a) => write_repr_pr(w, a)?,
        DebugEntryKind::ReprPl(a) => write_repr_pl(w, a)?,
        DebugEntryKind::ReprDecl(a) => write_repr_decl(w, a)?,
        DebugEntryKind::ReprRq(a) => write_repr_rq(w, a)?,
        DebugEntryKind::ReprPqEarly(a) => write_repr_pq_early(w, a)?,
        DebugEntryKind::ReprPq(a) => write_repr_pq(w, a)?,
        DebugEntryKind::ReprSqlParser(a) => write_repr_sql_parser(w, a)?,
        DebugEntryKind::ReprSql(a) => write_repr_sql(w, a)?,
        DebugEntryKind::NewStage(_) | DebugEntryKind::Message(_) => unreachable!(),
    }
    writeln!(w, "</div>")?;
    writeln!(w, "</details>")
}

fn write_repr_prql<W: Write>(w: &mut W, source_tree: &SourceTree) -> Result {
    writeln!(w, r#"<div class="prql repr">"#)?;

    write_key_values(w, &[("root", &source_tree.root)])?;

    let reverse_ids: HashMap<_, _> = source_tree
        .source_ids
        .iter()
        .map(|(id, path)| (path, id))
        .collect();

    for (path, source) in &source_tree.sources {
        writeln!(w, r#"<div class="source indent">"#)?;

        let source_id = reverse_ids.get(path).unwrap();
        write_key_values(w, &[("path", &path), ("source_id", source_id)])?;

        let source_escaped = escape_html(source);
        writeln!(
            w,
            r#"<code id="source-{source_id}">{source_escaped}</code>"#
        )?;
        writeln!(w, "</div>")?; // source
    }

    writeln!(w, "</div>")
}

fn write_repr_lr<W: Write>(w: &mut W, tokens: &lr::Tokens) -> Result {
    writeln!(w, r#"<table class="lr repr">"#)?;

    for token in &tokens.0 {
        writeln!(w, r#"<tr class="token">"#,)?;
        writeln!(w, "<td>{}</td>", escape_html(&format!("{:?}", token.kind)))?;
        writeln!(
            w,
            r#"<td><span class="span">{}:{}</span></td>"#,
            token.span.start, token.span.end
        )?;
        writeln!(w, "</tr>")?;
    }

    writeln!(w, "</table>")
}

fn write_repr_pr<W: Write>(w: &mut W, ast: &pr::ModuleDef) -> Result {
    writeln!(w, r#"<div class="pr repr">"#)?;

    let json = serde_json::to_string(ast).unwrap();
    let json_node: serde_json::Value = serde_json::from_str(&json).unwrap();
    write_json_ast_node(w, json_node, false)?;

    writeln!(w, "</div>")
}

fn write_repr_pl<W: Write>(w: &mut W, ast: &pl::ModuleDef) -> Result {
    writeln!(w, r#"<div class="pl repr">"#)?;

    let json = serde_json::to_string(ast).unwrap();
    let json_node: serde_json::Value = serde_json::from_str(&json).unwrap();
    write_json_ast_node(w, json_node, false)?;

    writeln!(w, "</div>")
}

fn write_repr_decl<W: Write>(w: &mut W, root_mod: &decl::RootModule) -> Result {
    writeln!(w, r#"<div class="decl repr">"#)?;

    for (name, decl) in root_mod.module.names.iter().sorted_by_key(|x| x.0.as_str()) {
        write_decl(w, decl, name, &root_mod.span_map)?;
    }

    writeln!(w, "</div>")
}

fn write_repr_rq<W: Write>(w: &mut W, ast: &rq::RelationalQuery) -> Result {
    writeln!(w, r#"<div class="rq repr">"#)?;

    let json = serde_json::to_string(ast).unwrap();
    let json_node: serde_json::Value = serde_json::from_str(&json).unwrap();
    write_json_ast_node(w, json_node, false)?;

    writeln!(w, "</div>")
}

fn write_repr_pq_early<W: Write>(w: &mut W, ast: &[pq_ast::SqlTransform]) -> Result {
    writeln!(w, r#"<div class="pq repr">"#)?;

    let json = serde_json::to_string(ast).unwrap();
    let json_node: serde_json::Value = serde_json::from_str(&json).unwrap();
    write_json_ast_node(w, json_node, false)?;

    writeln!(w, "</div>")
}

fn write_repr_pq<W: Write>(w: &mut W, ast: &pq_ast::SqlQuery) -> Result {
    writeln!(w, r#"<div class="pq repr">"#)?;

    let json = serde_json::to_string(ast).unwrap();
    let json_node: serde_json::Value = serde_json::from_str(&json).unwrap();
    write_json_ast_node(w, json_node, false)?;

    writeln!(w, "</div>")
}

fn write_repr_sql_parser<W: Write>(w: &mut W, ast: &sqlparser::ast::Query) -> Result {
    writeln!(w, r#"<div class="sql-parser repr">"#)?;

    let json = serde_json::to_string(ast).unwrap();
    let json_node: serde_json::Value = serde_json::from_str(&json).unwrap();
    write_json_ast_node(w, json_node, false)?;

    writeln!(w, "</div>")
}

fn write_repr_sql<W: Write>(w: &mut W, query: &str) -> Result {
    writeln!(w, r#"<div class="sql repr">"#)?;
    writeln!(w, "<pre><code>{}</code></pre>", query)?;
    writeln!(w, "</div>")
}

fn write_message<W: Write>(w: &mut W, message: &Message) -> Result {
    write!(w, "<div>[<b>{}</b>", message.level)?;
    if let Some(module_path) = &message.module_path {
        write!(w, r#" {}"#, module_path)?;
    }
    writeln!(w, "] {}", message.text)?;
    writeln!(w, "</div>")
}

fn write_key_values<W: Write>(w: &mut W, pairs: &[(&'static str, &dyn Debug)]) -> Result {
    writeln!(w, r#"<div class="key-values">"#)?;
    for (k, v) in pairs {
        writeln!(w, r#"<div><b class="blue">{k}</b>: {v:?}</div>"#)?;
    }
    writeln!(w, "</div>")
}

/// A hacky way to reconstruct AST nodes from their JSON serialization.
/// Finds structures that look like `{ "BinOp": { ... }, "span": ... }`
/// and converts them to `<div class=ast-node>...</div>`.
fn write_json_ast_node<W: Write>(
    w: &mut W,
    node: serde_json::Value,
    is_node_contents: bool,
) -> Result {
    match node {
        serde_json::Value::Null => write!(w, "None"),
        serde_json::Value::Bool(b) => write!(w, "{b}"),
        serde_json::Value::Number(n) => write!(w, "{n}"),
        serde_json::Value::String(s) => write!(w, "{s}"),
        serde_json::Value::Array(items) => {
            writeln!(w, r#"<ul class="json-array">"#)?;
            for item in items {
                write!(w, "<li>")?;
                write_json_ast_node(w, item, false)?;
                write!(w, "</li>")?;
            }
            writeln!(w, "</ul>")?;
            Ok(())
        }
        serde_json::Value::Object(properties) => {
            let is_ast_node = properties.contains_key("span")
                || properties.contains_key("id")
                || properties.contains_key("ty")
                || (properties.len() == 1
                    && !is_node_contents
                    && properties.values().next().unwrap().is_object());
            if is_ast_node {
                return write_ast_node_from_object(w, properties);
            }

            writeln!(w, r#"<div class="json-object">"#)?;
            for (key, value) in properties {
                write!(w, r#"<span>{key}: </span><div class="json-value">"#)?;
                write_json_ast_node(w, value, false)?;
                writeln!(w, "</div>")?;
            }
            writeln!(w, "</div>")
        }
    }
}

fn write_ast_node_from_object<W: Write>(
    w: &mut W,
    mut properties: serde_json::Map<String, serde_json::Value>,
) -> Result {
    let id: Option<i64> = properties.remove("id").and_then(|s| match s {
        serde_json::Value::Null => None,
        serde_json::Value::Number(n) if n.is_i64() => n.as_i64(),
        _ => unreachable!("expected id to be int, got: {}", s),
    });
    let span: Option<String> = properties.remove("span").and_then(|s| match s {
        serde_json::Value::Null => None,
        serde_json::Value::String(s) => Some(s),
        _ => unreachable!("expected span to be string, got: {}", s),
    });
    let ty: Option<serde_json::Value> = properties.remove("ty");

    let first_item = properties.into_iter().next();
    let (name, contents) =
        first_item.unwrap_or_else(|| ("<empty>".into(), serde_json::Value::Null));

    write!(w, r#"<div class=ast-node tabindex=2>"#)?;

    write!(w, "<div class=header>")?;

    let h2_id = id.map(|i| format!("id=ast-{i} ")).unwrap_or_default();
    write!(w, "<h2 {h2_id}class=clickable>{name}</h2>")?;

    if let Some(id) = id {
        write!(w, r#"<span>id={id}</span>"#)?;
    }
    if let Some(span) = span {
        write!(w, r#"<span class="span">{span}</span>"#)?;
    }
    if let Some(ty) = ty {
        let ty_json = ty.to_string();
        if let Ok(ty) = serde_json::from_str::<pr::Ty>(&ty_json) {
            let ty_prql = codegen::write_ty(&ty);
            write!(w, r#"<span class="ty">{ty_prql}</span>"#)?;
        }
    }
    write!(w, "</div>")?;

    write!(w, r#"<div class="contents indent">"#)?;
    write_json_ast_node(w, contents, true)?;
    write!(w, "</div>")?;
    write!(w, "</div>")
}

fn write_decl<W: Write>(
    w: &mut W,
    decl: &decl::Decl,
    name: &String,
    span_map: &HashMap<usize, pr::Span>,
) -> Result {
    write!(w, r#"<div class="ast-node" tabindex=2>"#)?;

    // header
    {
        write!(w, "<div class=header>")?;
        write!(w, r#"<h2 class="clickable blue">{name}</h2>"#)?;

        let span = decl.declared_at.as_ref().and_then(|id| span_map.get(id));
        if let Some(span) = span {
            write!(w, r#"<span class="span">{span:?}</span>"#)?;
        }
        write!(w, "</div>")?; // header
    }

    write!(w, r#"<div class="contents indent">"#)?;
    match &decl.kind {
        decl::DeclKind::Module(m) => {
            for (name, decl) in m.names.iter().sorted_by_key(|x| x.0.as_str()) {
                write_decl(w, decl, name, span_map)?;
            }
        }
        decl::DeclKind::Expr(expr) => {
            let json = serde_json::to_string(expr).unwrap();
            let json_node: serde_json::Value = serde_json::from_str(&json).unwrap();
            write_json_ast_node(w, json_node, false)?;
        }
        decl::DeclKind::Ty(ty) => {
            writeln!(w, r#"<span>{}</span>"#, escape_html(&codegen::write_ty(ty)))?;
        }
        decl::DeclKind::TableDecl(table_decl) => {
            let json = serde_json::to_string(table_decl).unwrap();
            let json_node: serde_json::Value = serde_json::from_str(&json).unwrap();
            write_json_ast_node(w, json_node, false)?;
        }
        _ => {
            write!(w, r#"<div>{}</div>"#, decl.kind)?;
        }
    }
    write!(w, "</div>")?; // contents
    write!(w, "</div>") // ast-node
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#039;")
}

const CSS_STYLES: &str = r#"
body {
    font-size: 12px;
    font-family: monospace;

    --background: #1F1F1F;
    --background-hover: #4D4F51;
    --background-focus: #623315;
    --text: #CACACA;
    --text-blue: #4ebffc;
    --text-green: #4BBFA7;
    --text-yellow: #DCDCAA;
    --text-muted: gray;

    background-color: var(--background);
    color: var(--text);
}

summary::marker {
    content: "";
}

.clickable {
    cursor: pointer;
    text-decoration: underline;
}

.highlight-hover {
    background-color: var(--background-hover);
}
.highlight-focus {
    background-color: var(--background-focus);
}
.yellow {
    color: var(--text-yellow);
}
.blue:not(#fakeId) {
    color: var(--text-blue);
}
.muted {
    color: var(--text-muted);
}

.key-values {
    display: flex;
    gap: 1em;
}

.entry {
    &.entry-label {
        margin: 0;
        display: block;
    }
    &.entry-collapse {
        display: none;
    }
    &.entry-collapse:checked + .entry-label + .entry-content {
        display: none;
    }
    &.entry-content {
        display: flex;
        flex-direction: column;
    }
}
.entry>h2 {
    border-bottom: solid;
    margin-top: 2rem;
    margin-bottom: 0.5rem;
}

code {
    white-space: preserve;
    word-break: keep-all;
}
.indent {
    padding-left: 1em;
    margin-top: 2px;
    border-left: 1px solid gray;
}
.span {
    color: var(--text-muted);
}
table.repr.lr {
    border-collapse: collapse;
}

.ast-node>.header {
    display: flex;
}
.ast-node>.header>h2 {
    font-size: inherit;
    color: var(--text-green);
    margin: 0;
}
.ast-node>.header>span {
    display: inline-block;
    margin-left: 1em;
}
.ast-node.collapsed>.header::after {
    content: '...';
    margin-left: 1em;
}
.ast-node.collapsed>.contents {
    display: none;
}
.ast-node>.contents.indent>.json-array,
.ast-node>.contents.indent>.json-object {
    padding-left: 0;
}
.ast-node:focus {
    background-color: var(--background-focus);
}

.json-array {
    margin: 0;
    list-style-type: "- ";
}
.json-array,.json-object,.json-value {
    padding-left: 1em;
}
"#;

const JS_SCRIPT: &str = r#"
const ast_node_mousedown = (event) => {
    event.stopPropagation();

    const ast_node = event.currentTarget;

    if (document.activeElement == ast_node) {
        // unfocus after click (and focusing) is finished
        setTimeout(() => {
            ast_node.blur();
            highlight(null, null, 'highlight-focus');
        }, 0)
    }
};

const ast_node_focus = (event) => {
    event.stopPropagation();

    const ast_node = event.currentTarget;

    const span_element = ast_node.querySelector(':scope > .header > .span');
    highlight(span_element, null, 'highlight-focus');
};

const ast_node_mouseover = (event) => {
    if (document.activeElement != document.body) {
        // something else has focus, we don't show hover highlight
        event.stopPropagation();
        return;
    }

    const ast_node = event.currentTarget;
    if (ast_node.classList.contains("highlight-hover")) {
        event.stopPropagation();
        return;
    }

    // find the span DOM node
    const span_element = ast_node.querySelector(':scope > .header > .span');
    if (!span_element) {
        // if there is no node, return without stopping propagation
        return;
    }

    event.stopPropagation();
    highlight(span_element, ast_node, 'highlight-hover');
}

const highlight = (span_element, origin_element, highlight_class) => {
    // remove all existing highlights
    document.querySelectorAll("." + highlight_class).forEach(e => {
        e.classList.remove(highlight_class);
    });

    // highlight origin element
    if (origin_element) {
        origin_element.classList.add(highlight_class);
    }

    // highlight source
    if (span_element) {
        const span = extract_span(span_element);
        const code_element = document.getElementById(`source-${span.source_id}`);
        if (!code_element) {
            console.error(`cannot find source with id=${span.source_id}`);
        } else {
            const source_text = code_element.innerText;

            const before = escape_html(source_text.substring(0, span.start));
            const selected = escape_html(source_text.substring(span.start, span.end));
            const after = escape_html(source_text.substring(span.end));
            code_element.innerHTML = `${before}<span class=${highlight_class}>${selected}</span>${after}`;
        }
    }
};

const escape_html = (text) => {
    return text
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;")
        .replace(/'/g, "&#039;");
}

const extract_span = (span_element) => {
    const parts = span_element.innerText.split(':');
    const source_id = Number.parseInt(parts[0]);
    const start_end = parts[1].split('-');
    const start = Number.parseInt(start_end[0]);
    const end = Number.parseInt(start_end[1]);
    return { source_id, start, end };
};

const ast_node_title_click = (event) => {
    event.stopPropagation();
    const ast_node = event.target.parentElement.parentElement;
    ast_node.classList.toggle("collapsed");
};

document.addEventListener('DOMContentLoaded', () => {
    const ast_nodes = document.querySelectorAll(".ast-node");
    ast_nodes.forEach(ast_node => {
        ast_node.addEventListener("mouseover", ast_node_mouseover);
        ast_node.addEventListener("mousedown", ast_node_mousedown);
        ast_node.addEventListener("focus", ast_node_focus);

        const h2 = ast_node.querySelector(":scope > .header > h2");
        h2.addEventListener("click", ast_node_title_click);
    });
});
"#;
