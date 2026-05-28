//! Compile-pass fixture: #[tracing::instrument] is accepted on free functions
//! that return bool.

fn main() {
    let _ = looks_like_footnote_ref("[^1]");
    let _ = ends_with_footnote_ref("word.[^1]");
}

#[tracing::instrument(level = "trace", ret)]
fn looks_like_footnote_ref(token: &str) -> bool {
    token.starts_with("[^") && token.ends_with(']')
}

#[tracing::instrument(level = "trace", ret)]
fn ends_with_footnote_ref(token: &str) -> bool {
    token.ends_with(']') && token.contains("[^")
}
