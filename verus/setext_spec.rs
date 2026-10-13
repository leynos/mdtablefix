//! Complete production Setext assembly, with arbitrary retained quote prefixes.

use vstd::prelude::*;
use crate::*;

verus! {

/// Unicode White_Space scalars used by Rust's `char::is_whitespace`.
/// For example, `unicode_whitespace('\u{00a0}')` is true.
pub open spec fn unicode_whitespace(c: char) -> bool {
    ('\u{0009}' <= c <= '\u{000d}') || c == '\u{0020}' || c == '\u{0085}'
        || c == '\u{00a0}' || c == '\u{1680}' || ('\u{2000}' <= c <= '\u{200a}')
        || c == '\u{2028}' || c == '\u{2029}' || c == '\u{202f}'
        || c == '\u{205f}' || c == '\u{3000}'
}

/// Selects the prefix separator; for example, `seq!['>']` needs one space.
pub open spec fn needs_space_after(prefix: Seq<char>) -> bool {
    prefix.len() > 0 && !unicode_whitespace(prefix[prefix.len() - 1])
}

/// Adds a necessary prefix separator; for example, `seq!['>']` becomes `seq!['>', ' ']`.
pub open spec fn separated_prefix(prefix: Seq<char>) -> Seq<char> {
    if needs_space_after(prefix) { prefix.push(' ') } else { prefix }
}

/// Prefix admission depends on a paragraph probe, never on emitted ATX output.
/// For example, `seq!['>', ' ', '>']` is admitted and four leading spaces are refused.
pub open spec fn valid_prefix(prefix: Seq<char>) -> bool {
    let p = separated_prefix(prefix);
    let probe = p.push('x');
    spec_classify(probe, canonical_context()) == LineClass::ParagraphText
        && 0 <= spec_line_parts(probe).0 <= p.len()
        && spec_trim_start(probe, spec_line_parts(probe).0, probe.len() as int) == p.len()
}

/// Models the full emitted sequence; for example, prefix `>`, level two, and text `T` yield `> ## T`.
pub open spec fn assembled_heading(prefix: Seq<char>, level: int, text: Seq<char>) -> Seq<char> {
    separated_prefix(prefix)
        + (if level == 2 { seq!['#', '#', ' '] } else { seq!['#', ' '] }) + text
}

/// Indentation scanning cannot distinguish equal prefixes ending in non-indent scalars.
/// For example, scanning ` x` and ` #` at column zero yields width one and cursor one.
proof fn lemma_indentation_congruent(a: Seq<char>, b: Seq<char>, end: int,
    cursor: int, column: int, width: int)
    requires
        0 <= cursor <= end < a.len(), end < b.len(), 0 <= column < 4, 0 <= width,
        forall|i: int| 0 <= i < end ==> a[i] == b[i],
        !spec_is_markdown_whitespace(a[end]), !spec_is_markdown_whitespace(b[end]),
    ensures spec_indentation_at(a, cursor, column, width)
        == spec_indentation_at(b, cursor, column, width),
        cursor <= spec_indentation_at(a, cursor, column, width).1 <= end,
        0 <= spec_indentation_at(a, cursor, column, width).0 <= 4,
    decreases end - cursor
{
    if cursor < end && width < 4 {
        if a[cursor] == ' ' {
            lemma_indentation_congruent(a, b, end, cursor + 1, column, width + 1);
        } else if a[cursor] == '\t' {
            lemma_indentation_congruent(a, b, end, cursor + 1, column,
                width + 4 - ((column + width) % 4));
        }
    }
}

/// Arbitrarily many nested quotes consume the same retained prefix in both lines.
/// For example, `> > x` and `> > #` both begin their structural body at scalar four.
proof fn lemma_parts_congruent(a: Seq<char>, b: Seq<char>, end: int,
    cursor: int, column: int, fuel_a: nat, fuel_b: nat)
    requires
        0 <= cursor <= end < a.len(), end < b.len(), 0 <= column < 4,
        fuel_a > end - cursor, fuel_b > end - cursor,
        forall|i: int| 0 <= i < end ==> a[i] == b[i],
        !spec_is_markdown_whitespace(a[end]), !spec_is_markdown_whitespace(b[end]),
        a[end] != '>', b[end] != '>',
    ensures spec_line_parts_from(a, cursor, column, fuel_a)
        == spec_line_parts_from(b, cursor, column, fuel_b),
    decreases end - cursor
{
    lemma_indentation_congruent(a, b, end, cursor, column, 0);
    let (width, after) = spec_indentation_at(a, cursor, column, 0);
    if width < 4 && a[after] == '>' {
        assert(after < end);
        let next = after + 1;
        let next_column = (column + width + 1) % 4;
        if a[next] == ' ' {
            assert(next < end);
            lemma_parts_congruent(a, b, end, next + 1, (next_column + 1) % 4,
                (fuel_a - 1) as nat, (fuel_b - 1) as nat);
        } else {
            lemma_parts_congruent(a, b, end, next, next_column,
                (fuel_a - 1) as nat, (fuel_b - 1) as nat);
        }
    }
}

/// Composes outer indentation with nested quote scanning across a shared prefix.
/// For example, `  > x` and `  > #` both have body start four and are nonliteral.
proof fn lemma_line_parts_congruent(a: Seq<char>, b: Seq<char>, end: int)
    requires
        0 <= end < a.len(), end < b.len(),
        forall|i: int| 0 <= i < end ==> a[i] == b[i],
        !spec_is_markdown_whitespace(a[end]), !spec_is_markdown_whitespace(b[end]),
        a[end] != '>', b[end] != '>',
    ensures spec_line_parts(a) == spec_line_parts(b)
{
    lemma_indentation_congruent(a, b, end, 0, 0, 0);
    let (width, cursor) = spec_indentation_at(a, 0, 0, 0);
    if width < 4 {
        lemma_parts_congruent(a, b, end, cursor, width,
            (a.len() - cursor) as nat, (b.len() - cursor) as nat);
    }
}

/// Trimming equal prefix whitespace stops at the same first non-whitespace scalar.
/// For example, `  x` and `  #` both trim to scalar two.
proof fn lemma_trim_start_congruent(a: Seq<char>, b: Seq<char>, end: int, start: int)
    requires
        0 <= start <= end < a.len(), end < b.len(),
        forall|i: int| 0 <= i < end ==> a[i] == b[i],
        !spec_is_markdown_whitespace(a[end]), !spec_is_markdown_whitespace(b[end]),
    ensures spec_trim_start(a, start, a.len() as int)
        == spec_trim_start(b, start, b.len() as int),
    decreases end - start
{
    if start < end && spec_is_markdown_whitespace(a[start]) {
        lemma_trim_start_congruent(a, b, end, start + 1);
    }
}

/// A non-whitespace marker survives trailing-whitespace removal.
/// For example, trimming `##   ` retains both hashes and therefore has end two.
proof fn lemma_trim_keeps_marker(s: Seq<char>, first: int, marker: int, end: int)
    requires 0 <= first <= marker < end <= s.len(),
        !spec_is_markdown_whitespace(s[marker])
    ensures marker < spec_trim_end(s, first, end) <= end
    decreases end - marker
{
    if end > marker + 1 && spec_is_markdown_whitespace(s[end - 1]) {
        lemma_trim_keeps_marker(s, first, marker, end - 1);
    }
}

/// Complete construction theorem: prefix, separator, both levels, and arbitrary text.
/// For example, admitted prefix `>>` and text `T` yield ATX lines `>> # T` and `>> ## T`.
pub proof fn lemma_complete_heading_classifies(prefix: Seq<char>, level: int, text: Seq<char>)
    requires valid_prefix(prefix), level == 1 || level == 2,
    ensures
        spec_classify(assembled_heading(prefix, level, text), canonical_context())
            == LineClass::AtxHeading,
        spec_classify(assembled_heading(prefix, level, text), canonical_context())
            != LineClass::ParagraphText,
        forall|ctx: ClassifyCtxView| !ctx.is_in_fence ==>
            #[trigger] spec_classify(assembled_heading(prefix, level, text), ctx)
                == LineClass::AtxHeading,
        assembled_heading(prefix, level, text) ==
            (if level == 2 { separated_prefix(prefix).push('#').push('#').push(' ') }
             else { separated_prefix(prefix).push('#').push(' ') }) + text,
{
    let p = separated_prefix(prefix);
    let probe = p.push('x');
    let output = assembled_heading(prefix, level, text);
    assert(output =~=
        (if level == 2 { p.push('#').push('#').push(' ') }
         else { p.push('#').push(' ') }) + text);
    let boundary = p.len() as int;
    assert(forall|i: int| 0 <= i < boundary ==> probe[i] == output[i]);
    assert(output[boundary] == '#');
    lemma_line_parts_congruent(probe, output, boundary);
    let start = spec_line_parts(probe).0;
    assert(0 <= start <= boundary);
    lemma_trim_start_congruent(probe, output, boundary, start);
    assert(spec_trim_start(output, start, output.len() as int) == boundary);
    assert(!spec_is_blank_from(output, start)) by {
        assert(!spec_is_markdown_whitespace(output[boundary]));
    }
    assert(!spec_is_blank_from(output, 0));
    lemma_trim_keeps_marker(output, boundary, boundary, output.len() as int);
    let end = spec_trim_end(output, boundary, output.len() as int);
    if level == 1 {
        if end > boundary + 1 {
            assert(spec_marker_run_len(output, boundary + 1, end, '#') == 0);
        }
        assert(spec_marker_run_len(output, boundary, end, '#') == 1);
    } else {
        assert(end > boundary + 1) by {
            lemma_trim_keeps_marker(output, boundary, boundary + 1, output.len() as int);
        }
        if end > boundary + 2 {
            assert(spec_marker_run_len(output, boundary + 2, end, '#') == 0);
        }
        assert(spec_marker_run_len(output, boundary + 1, end, '#') == 1);
        assert(spec_marker_run_len(output, boundary, end, '#') == 2);
    }
    assert(spec_atx_heading(output, start));
    assert(!spec_fence_marker(output, start));
}

/// Concrete prefix witnesses exclude vacuous admission and exercise both levels.
/// For example, the empty prefix and `> >` are admitted and emit ATX headings at both levels.
pub proof fn lemma_setext_prefix_exists()
    ensures valid_prefix(Seq::<char>::empty()), valid_prefix(seq!['>', ' ', '>']),
        valid_prefix(seq![' ', ' ', ' ']), valid_prefix(seq!['>'])
{
    let plain = seq!['x'];
    assert(spec_line_parts(plain) == (0int, false));
    assert(spec_trimmed_range(plain, 0) == (0int, 1int));
    assert(spec_classify(plain, canonical_context()) == LineClass::ParagraphText);
    assert(valid_prefix(Seq::<char>::empty()));

    let nested_prefix = seq!['>', ' ', '>'];
    let nested = seq!['>', ' ', '>', ' ', 'x'];
    assert(separated_prefix(nested_prefix) == seq!['>', ' ', '>', ' ']);
    assert(separated_prefix(nested_prefix).push('x') =~= nested);
    assert(spec_indentation_at(nested, 0, 0, 0) == (0int, 0int));
    assert(spec_indentation_at(nested, 2, 2, 0) == (0int, 2int));
    assert(spec_indentation_at(nested, 4, 0, 0) == (0int, 4int));
    assert(spec_line_parts_from(nested, 4, 0, 3) == (4int, false));
    assert(spec_line_parts_from(nested, 2, 2, 4) == (4int, false));
    assert(spec_line_parts_from(nested, 0, 0, 5) == (4int, false));
    assert(spec_line_parts(nested) == (4int, false));
    assert(spec_trimmed_range(nested, 4) == (4int, 5int));
    assert(spec_classify(nested, canonical_context()) == LineClass::ParagraphText);
    assert(valid_prefix(nested_prefix));
    let indented_prefix = seq![' ', ' ', ' '];
    let indented = indented_prefix.push('x');
    assert(spec_indentation_at(indented, 3, 0, 3) == (3int, 3int));
    assert(spec_indentation_at(indented, 2, 0, 2) == (3int, 3int));
    assert(spec_indentation_at(indented, 1, 0, 1) == (3int, 3int));
    assert(spec_indentation_at(indented, 0, 0, 0) == (3int, 3int));
    assert(spec_line_parts(indented) == (3int, false));
    assert(spec_trimmed_range(indented, 3) == (3int, 4int));
    assert(spec_classify(indented, canonical_context()) == LineClass::ParagraphText);
    assert(valid_prefix(indented_prefix));
    let quoted_prefix = seq!['>'];
    let quoted = seq!['>', ' ', 'x'];
    assert(separated_prefix(quoted_prefix).push('x') =~= quoted);
    assert(spec_indentation_at(quoted, 0, 0, 0) == (0int, 0int));
    assert(spec_indentation_at(quoted, 2, 2, 0) == (0int, 2int));
    assert(spec_line_parts_from(quoted, 2, 2, 2) == (2int, false));
    assert(spec_line_parts_from(quoted, 0, 0, 3) == (2int, false));
    assert(spec_line_parts(quoted) == (2int, false));
    assert(spec_trimmed_range(quoted, 2) == (2int, 3int));
    assert(spec_classify(quoted, canonical_context()) == LineClass::ParagraphText);
    assert(valid_prefix(quoted_prefix));
    lemma_complete_heading_classifies(Seq::<char>::empty(), 1, seq!['T']);
    lemma_complete_heading_classifies(Seq::<char>::empty(), 2, seq!['T']);
    lemma_complete_heading_classifies(nested_prefix, 1, seq!['T']);
    lemma_complete_heading_classifies(nested_prefix, 2, seq!['T']);
    lemma_complete_heading_classifies(indented_prefix, 1, seq!['T']);
    lemma_complete_heading_classifies(indented_prefix, 2, seq!['T']);
    lemma_complete_heading_classifies(quoted_prefix, 1, seq!['T']);
    lemma_complete_heading_classifies(quoted_prefix, 2, seq!['T']);
}

} // verus!
