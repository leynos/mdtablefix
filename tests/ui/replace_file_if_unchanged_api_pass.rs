//! Compile-pass fixture: the conditional replacement API keeps its public
//! signature for downstream callers holding a directory capability.

use camino::Utf8Path;
use cap_std::fs_utf8::Dir;

fn main() {
    let _replace: fn(&Dir, &Utf8Path, &str, &str) -> std::io::Result<bool> =
        mdtablefix::io::replace_file_if_unchanged;
}
