//! Compile-pass fixture: `mdtablefix::io::replace_file` stays callable by
//! downstream code that holds a `cap_std::fs_utf8::Dir` capability.

use camino::Utf8Path;
use cap_std::fs_utf8::Dir;

fn main() {
    // The documented signature is pinned by coercing the function item to a
    // function pointer; calling it needs a real directory capability, which
    // the library deliberately does not open for the caller.
    let _replace: fn(&Dir, &Utf8Path, &str) -> std::io::Result<()> = mdtablefix::io::replace_file;
}
