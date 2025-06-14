use html5ever::driver::ParseOpts;
use html5ever::{parse_document, tendril::TendrilSink};
use markup5ever_rcdom::{Handle, NodeData, RcDom};

use crate::{FENCE_RE, TABLE_END_RE, TABLE_START_RE};

fn node_text(handle: &Handle) -> String {
    let mut parts = Vec::new();
    collect_text(handle, &mut parts);
    parts
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn collect_text(handle: &Handle, out: &mut Vec<String>) {
    match &handle.data {
        NodeData::Text { contents } => out.push(contents.borrow().to_string()),
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
                collect_text(child, out);
            }
        }
        NodeData::Document => {
            for child in handle.children.borrow().iter() {
                collect_text(child, out);
            }
        }
        _ => {}
    }
}

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
        let mut header_row = false;
        for child in row.children.borrow().iter() {
            if let NodeData::Element { name, .. } = &child.data {
                if name.local.as_ref() == "td" || name.local.as_ref() == "th" {
                    if name.local.as_ref() == "th" {
                        header_row = true;
                    }
                    cells.push(node_text(child));
                }
            }
        }
        if i == 0 {
            first_header = header_row;
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

pub(crate) fn html_table_to_markdown(lines: &[String]) -> Vec<String> {
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

pub fn convert_html_tables(lines: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut buf = Vec::new();
    let mut depth = 0usize;
    let mut in_html = false;
    let mut in_code = false;

    for line in lines {
        if FENCE_RE.is_match(line) {
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
            buf.push(line.trim_end().to_string());
            depth += TABLE_START_RE.find_iter(line).count();
            if TABLE_END_RE.is_match(line) {
                depth = depth.saturating_sub(TABLE_END_RE.find_iter(line).count());
                if depth == 0 {
                    out.extend(html_table_to_markdown(&buf));
                    buf.clear();
                    in_html = false;
                }
            }
            continue;
        }

        if TABLE_START_RE.is_match(line.trim_start()) {
            in_html = true;
            depth = 0;
            buf.push(line.trim_end().to_string());
            depth += TABLE_START_RE.find_iter(line).count();
            if TABLE_END_RE.is_match(line) {
                depth = depth.saturating_sub(TABLE_END_RE.find_iter(line).count());
                if depth == 0 {
                    out.extend(html_table_to_markdown(&buf));
                    buf.clear();
                    in_html = false;
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
