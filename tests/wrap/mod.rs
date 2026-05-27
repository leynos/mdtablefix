//! Integration tests for text wrapping behaviour in Markdown content.
//!
//! This module validates the wrapping functionality of the `mdtablefix` tool,
//! grouped by feature for clarity.

use mdtablefix::process_stream;

#[macro_use]
#[path = "../common/mod.rs"]
mod common;
use common::{assert_wrapped_blockquote, assert_wrapped_list_item, run_cli_with_stdin};

mod paragraphs;
mod lists;
mod footnotes;
mod link_reference_definitions;
mod link_ref_snapshots;
mod blockquotes;
mod hard_line_breaks;
mod links;
mod fence_behaviour;
mod cli;
