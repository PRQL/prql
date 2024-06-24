use std::collections::HashMap;
use std::fmt::{Debug, Result, Write};

use crate::SourceTree;

use super::log::*;
use crate::ir::{decl, pl, rq};
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
    writeln!(w, "<!DOCTYPE html>")?;

    writeln!(w, "<head>")?;
    writeln!(w, "<title>prqlc debug log {}</title>", debug_log.started_at)?;
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

    // entries
    writeln!(w, "<div class=entries>")?;
    for (index, entry) in debug_log.entries.iter().enumerate() {
        writeln!(w, r#"<div class="entry">"#,)?;

        match &entry.kind {
            DebugEntryKind::NewStage(stage) => {
                let substage = stage
                    .sub_stage()
                    .map(|s| s.to_lowercase())
                    .unwrap_or_default();
                let stage = stage.as_ref().to_lowercase();

                writeln!(w, "<br/><span class=muted>{stage} {substage}</span><hr/>")?;
            }
            DebugEntryKind::Message(message) => {
                write_message(w, message)?;
            }
            _ => {
                write_titled_entry(w, entry, index)?;
            }
        }

        writeln!(w, "</div>")?; // entry
    }
    writeln!(w, "</div>")?; // entries

    writeln!(w, "</body>")?;

    Ok(())
}

fn write_titled_entry<W: Write>(w: &mut W, entry: &DebugEntry, index: usize) -> Result {
    let entry_id = format!("entry-{index}");

    writeln!(
        w,
        "<input id={entry_id} class=entry-collapse type=checkbox>"
    )?;

    writeln!(w, r#"<label for={entry_id} class="entry-label clickable">"#)?;
    let kind = entry.kind.as_ref()[4..].to_ascii_uppercase();
    writeln!(w, r#"[<b>AST</b>] <span class=yellow>{kind}</span>"#,)?;
    writeln!(w, r#"</label>"#)?;
    writeln!(w, r#"<div class="entry-content">"#)?;
    match &entry.kind {
        DebugEntryKind::ReprPrql(a) => write_repr_prql(w, a)?,
        DebugEntryKind::ReprLr(a) => write_repr_lr(w, a)?,
        DebugEntryKind::ReprPr(a) => write_repr_pr(w, a)?,
        DebugEntryKind::ReprPl(a) => write_repr_pl(w, a)?,
        DebugEntryKind::ReprDecl(a) => write_repr_decl(w, a)?,
        DebugEntryKind::ReprRq(a) => write_repr_rq(w, a)?,
        DebugEntryKind::ReprSql(a) => write_repr_sql(w, a)?,
        DebugEntryKind::NewStage(_) | DebugEntryKind::Message(_) => unreachable!(),
    }
    writeln!(w, "</div>")
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

        writeln!(w, "<code id=source-{source_id}>{source}</code>")?;
        writeln!(w, "</div>")?; // source
    }

    writeln!(w, "</div>")
}

fn write_repr_lr<W: Write>(w: &mut W, tokens: &lr::Tokens) -> Result {
    writeln!(w, r#"<div class="lr repr">"#)?;

    for token in &tokens.0 {
        writeln!(
            w,
            r#"<token span="{}:{}">"#,
            token.span.start, token.span.end
        )?;
        writeln!(w, "{:?}", token.kind)?;
        writeln!(w, "</token>")?;
    }

    writeln!(w, "</div>")
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

fn write_repr_decl<W: Write>(w: &mut W, ast: &decl::RootModule) -> Result {
    writeln!(w, r#"<div class="decl repr">"#)?;

    let json = serde_json::to_string(ast).unwrap();
    let json_node: serde_json::Value = serde_json::from_str(&json).unwrap();
    write_json_ast_node(w, json_node, false)?;

    writeln!(w, "</div>")
}

fn write_repr_rq<W: Write>(w: &mut W, ast: &rq::RelationalQuery) -> Result {
    writeln!(w, r#"<div class="rq repr">"#)?;

    let json = serde_json::to_string(ast).unwrap();
    let json_node: serde_json::Value = serde_json::from_str(&json).unwrap();
    write_json_ast_node(w, json_node, false)?;

    writeln!(w, "</div>")
}

fn write_repr_sql<W: Write>(w: &mut W, query: &str) -> Result {
    writeln!(w, r#"<div class="sql repr">"#)?;
    writeln!(w, "<code>{}</code>", query)?;
    writeln!(w, "</div>")
}

fn write_message<W: Write>(w: &mut W, message: &Message) -> Result {
    write!(w, r#"<div>[<b>{}</b>"#, message.level)?;
    if let Some(module_path) = &message.module_path {
        write!(w, r#" {}"#, module_path)?;
    }
    writeln!(w, "] {}", message.text)?;
    writeln!(w, "</div>")
}

fn write_key_values<W: Write>(w: &mut W, pairs: &[(&'static str, &dyn Debug)]) -> Result {
    writeln!(w, r#"<div class="key-values">"#)?;
    for (k, v) in pairs {
        writeln!(w, "<div><b class=blue>{k}</b>: {v:?}</div>")?;
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
            write!(w, r#"<ul class=json-array>"#)?;
            for item in items {
                write!(w, "<li>")?;
                write_json_ast_node(w, item, false)?;
                write!(w, "</li>")?;
            }
            write!(w, "</ul>")?;
            Ok(())
        }
        serde_json::Value::Object(mut map) => {
            let span = map.remove("span").and_then(|s| match s {
                serde_json::Value::Null => None,
                serde_json::Value::String(s) => Some(s),
                _ => unreachable!("expected span to be string, got: {}", s),
            });

            if !is_node_contents && map.len() == 1 {
                let (name, inner) = map.into_iter().next().unwrap();
                return write_ast_node(w, &name, span, inner);
            }

            write!(w, r#"<div class=json-object>"#)?;
            for (key, value) in map {
                write!(w, "<span>{key}: </span><div class=json-value>")?;
                write_json_ast_node(w, value, false)?;
                write!(w, "</div>")?;
            }
            write!(w, "</div>")
        }
    }
}

fn write_ast_node<W: Write>(
    w: &mut W,
    name: &str,
    span: Option<String>,
    contents: serde_json::Value,
) -> Result {
    write!(w, r#"<div class=ast-node tabindex=2>"#)?;

    write!(w, "<div class=header>")?;
    write!(w, "<h2 class=clickable>{name}</h2>")?;
    if let Some(span) = span {
        write!(w, r#"<span class="span">{span}</span>"#)?;
    }
    write!(w, "</div>")?;

    write!(w, r#"<div class="contents indent">"#)?;
    write_json_ast_node(w, contents, true)?;
    write!(w, "</div>")?;
    write!(w, "</div>")
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
.blue {
    color: var(--text-blue);
}
.muted {
    color: var(--text-muted);
}

.key-values {
    display: flex;
    gap: 1em;
}

.entries {
    direction: flex;
    flex-direction: column;
}
.entry-label {
    margin: 0;

    display: block;
}
.entry-collapse {
    display: none;
}
.entry-collapse:checked + .entry-label + .entry-content {
    display: none;
}
.entry-content {
    display: flex;
    flex-direction: column;
}

code {
    white-space: preserve;
    word-break: keep-all;
}
.indent {
    padding-left: 1em;
    border-left: 1px solid gray;
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
    color: var(--text-muted);
    display: inline-block;
    margin-left: 1em;
    font-weight: normal;
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

            const before = source_text.substring(0, span.start);
            const selected = source_text.substring(span.start, span.end);
            const after = source_text.substring(span.end);
            code_element.innerHTML = `${before}<span class=${highlight_class}>${selected}</span>${after}`;
        }
    }
};

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
