//! Utilities for converting HTML tables embedded in Markdown into
//! Markdown table syntax.
//!
//! The conversion is intentionally simple: only `<table>`, `<tr>`,
//! `<th>`, and `<td>` tags are recognised. Attributes and tag casing
//! are ignored. The resulting Markdown lines are passed to
//! `reflow_table` to ensure consistent column widths.

use html5ever::driver::ParseOpts;
use html5ever::{parse_document, tendril::TendrilSink};
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use regex::Regex;
use std::sync::LazyLock;

use crate::is_fence;

static TABLE_START_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^<table(?:\s|>|$)").unwrap());
static TABLE_END_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)</table>").unwrap());

/// Extracts the text content of a DOM node, collapsing consecutive
/// whitespace to single spaces.
fn node_text(handle: &Handle) -> String {
    let mut out = String::new();
    let mut last_space = false;
    collect_text(handle, &mut out, &mut last_space);
    out.trim().to_string()
}

/// Recursively appends text nodes from `handle` to `out`, tracking whether the
/// previous output was whitespace.
fn collect_text(handle: &Handle, out: &mut String, last_space: &mut bool) {
    match &handle.data {
        NodeData::Text { contents } => {
            for ch in contents.borrow().chars() {
                if ch.is_whitespace() {
                    *last_space = true;
                } else {
                    if *last_space && !out.is_empty() {
                        out.push(' ');
                    }
                    out.push(ch);
                    *last_space = false;
                }
            }
        }
        NodeData::Element { name, .. } => {
            let tag = name.local.as_ref();
            if tag.eq_ignore_ascii_case("script")
                || tag.eq_ignore_ascii_case("style")
                || tag.eq_ignore_ascii_case("noscript")
                || tag.eq_ignore_ascii_case("template")
                || tag.eq_ignore_ascii_case("head")
            {
                return;
            }
            for child in handle.children.borrow().iter() {
                collect_text(child, out, last_space);
            }
        }
        NodeData::Document => {
            for child in handle.children.borrow().iter() {
                collect_text(child, out, last_space);
            }
        }
        _ => {}
    }
}

/// Walks the DOM tree collecting `<table>` nodes under `handle`.
fn collect_tables(handle: &Handle, tables: &mut Vec<Handle>) {
    if let NodeData::Element { name, .. } = &handle.data {
        if name.local.as_ref() == "table" {
            tables.push(handle.clone());
        }
    }
    for child in handle.children.borrow().iter() {
        collect_tables(child, tables);
    }
}

/// Collects all `<tr>` nodes beneath `handle`.
fn collect_rows(handle: &Handle, rows: &mut Vec<Handle>) {
    if let NodeData::Element { name, .. } = &handle.data {
        if name.local.as_ref() == "tr" {
            rows.push(handle.clone());
        }
    }
    for child in handle.children.borrow().iter() {
        collect_rows(child, rows);
    }
}

/// Returns `true` if `handle` contains a `<b>` or `<strong>` descendant.
fn contains_strong(handle: &Handle) -> bool {
    if let NodeData::Element { name, .. } = &handle.data {
        let tag = name.local.as_ref();
        if tag.eq_ignore_ascii_case("strong") || tag.eq_ignore_ascii_case("b") {
            return true;
        }
    }
    let children = handle.children.borrow();
    children.iter().any(contains_strong)
}

/// Converts a `<table>` DOM node into Markdown table lines and calls
/// `reflow_table` so the columns are uniformly padded.
fn table_node_to_markdown(table: &Handle) -> Vec<String> {
    let mut row_handles = Vec::new();
    collect_rows(table, &mut row_handles);
    if row_handles.is_empty() {
        return Vec::new();
    }

    let mut out = Vec::new();
    let mut first_header = false;
    let mut col_count = 0;
    for (i, row) in row_handles.iter().enumerate() {
        let mut cells = Vec::new();
        let mut all_header = true;
        for child in row.children.borrow().iter() {
            if let NodeData::Element { name, .. } = &child.data {
                if name.local.as_ref() == "td" || name.local.as_ref() == "th" {
                    let is_header = if name.local.as_ref() == "th" {
                        true
                    } else {
                        contains_strong(child)
                    };
                    all_header &= is_header;
                    cells.push(node_text(child));
                }
            }
        }
        if i == 0 {
            first_header = all_header;
            col_count = cells.len();
        }
        out.push(format!("| {} |", cells.join(" | ")));
    }
    if first_header {
        let sep: Vec<String> = (0..col_count).map(|_| "---".to_string()).collect();
        out.insert(1, format!("| {} |", sep.join(" | ")));
    }
    crate::reflow_table(&out)
}

/// Parses HTML table markup and returns the equivalent Markdown lines.
///
/// If no `<table>` elements are present, the input is returned unchanged.
fn table_lines_to_markdown(lines: &[String]) -> Vec<String> {
    let indent: String = lines
        .first()
        .map(|l| l.chars().take_while(|c| c.is_whitespace()).collect())
        .unwrap_or_default();
    let html: String = lines
        .iter()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n");
    let opts = ParseOpts::default();
    let dom: RcDom = parse_document(RcDom::default(), opts).one(html);

    let mut tables = Vec::new();
    collect_tables(&dom.document, &mut tables);
    if tables.is_empty() {
        return lines.to_vec();
    }

    let mut out = Vec::new();
    for table in tables {
        for line in table_node_to_markdown(&table) {
            out.push(format!("{indent}{line}"));
        }
    }
    out
}

/// Buffers a single line of HTML, updating nesting depth and emitting completed
/// tables when an end tag is encountered.
fn push_html_line(
    line: &str,
    buf: &mut Vec<String>,
    depth: &mut usize,
    in_html: &mut bool,
    out: &mut Vec<String>,
) {
    buf.push(line.trim_end().to_string());
    *depth += TABLE_START_RE.find_iter(line).count();
    if TABLE_END_RE.is_match(line) {
        *depth = depth.saturating_sub(TABLE_END_RE.find_iter(line).count());
        if *depth == 0 {
            out.extend(html_table_to_markdown(buf));
            buf.clear();
            *in_html = false;
        }
    }
}

/// Converts any HTML tables in `lines` to Markdown syntax.
pub(crate) fn html_table_to_markdown(lines: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut buf = Vec::new();
    let mut depth = 0usize;

    for line in lines {
        if depth > 0 || TABLE_START_RE.is_match(line.trim_start()) {
            buf.push(line.trim_end().to_string());
            depth += TABLE_START_RE.find_iter(line).count();
            if TABLE_END_RE.is_match(line) {
                depth = depth.saturating_sub(TABLE_END_RE.find_iter(line).count());
                if depth == 0 {
                    out.extend(table_lines_to_markdown(&buf));
                    buf.clear();
                }
            }
            continue;
        }

        out.push(line.trim_end().to_string());
    }

    if !buf.is_empty() {
        out.extend(buf);
    }

    out
}

/// Processes Markdown lines and converts embedded HTML tables to Markdown.
///
/// Fenced code blocks are left untouched, allowing raw HTML examples to be
/// documented without modification.
#[must_use]
pub fn convert_html_tables(lines: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut buf = Vec::new();
    let mut depth = 0usize;
    let mut in_html = false;
    let mut in_code = false;

    for line in lines {
        if is_fence(line) {
            if in_html {
                out.append(&mut buf);
                in_html = false;
                depth = 0;
            }
            in_code = !in_code;
            out.push(line.trim_end().to_string());
            continue;
        }

        if in_code {
            out.push(line.trim_end().to_string());
            continue;
        }

        if in_html {
            push_html_line(line, &mut buf, &mut depth, &mut in_html, &mut out);
            continue;
        }

        if TABLE_START_RE.is_match(line.trim_start()) {
            in_html = true;
            push_html_line(line, &mut buf, &mut depth, &mut in_html, &mut out);
            continue;
        }

        out.push(line.trim_end().to_string());
    }

    if !buf.is_empty() {
        out.extend(buf);
    }

    out
}
