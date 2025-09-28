//! Integration tests for text wrapping behaviour in Markdown content.
//!
//! This module validates the wrapping functionality of the `mdtablefix` tool,
//! grouped by feature for clarity.

use mdtablefix::process_stream;

#[macro_use]
mod prelude;
use prelude::*;

mod paragraphs;
mod lists;
mod footnotes;
mod blockquotes;
mod hard_line_breaks;
mod links;
mod fence_behaviour;
mod cli;
