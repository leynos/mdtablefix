//! Compile-pass fixture: downstream callers can opt into list renumbering
//! through the public `Options` re-export and `process_stream_opts` API.

use mdtablefix::{Options, process_stream_opts};

fn main() {
    let input = vec!["5. An ordered item".to_string()];
    let options = Options {
        renumber: true,
        ..Default::default()
    };
    let output = process_stream_opts(&input, options);

    assert_eq!(output, vec!["1. An ordered item"]);
}
