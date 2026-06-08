//! Integration tests for text wrapping behaviour in Markdown content.
//!
//! This module validates the wrapping functionality of the `mdtablefix` tool,
//! grouped by feature for clarity.

use mdtablefix::process_stream;

#[macro_use]
#[path = "../common/mod.rs"]
mod common;

#[path = "../support/cli_stdin.rs"]
mod cli_stdin;
#[path = "../support/wrap_assertions.rs"]
mod wrap_assertions;

mod blockquotes;
mod checklist_code_spans;
mod cli;
mod cli_issue_329_property;
mod date_snapshots;
mod fence_behaviour;
mod footnotes;
mod hard_line_breaks;
mod inline_code_suffix_snapshots;
mod link_ref_snapshots;
mod link_reference_definitions;
mod links;
mod lists;
mod paragraphs;
mod spanning_code_spans;
mod tokenize_markdown;
