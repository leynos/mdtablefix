use mdtablefix::{lists::pop_counters_upto, renumber_lists};

macro_rules! lines_vec {
    ($($line:expr),* $(,)?) => {
        vec![$($line.to_string()),*]
    };
}

#[test]
fn pop_counters_removes_deeper_levels() {
    let mut counters = vec![(0usize, 1usize), (4, 2), (8, 3)];
    pop_counters_upto(&mut counters, 4);
    assert_eq!(counters, vec![(0, 1)]);
}

#[test]
fn pop_counters_no_change_when_indent_deeper() {
    let mut counters = vec![(0usize, 1usize), (4, 2)];
    pop_counters_upto(&mut counters, 6);
    assert_eq!(counters, vec![(0, 1), (4, 2)]);
}

#[test]
fn restart_after_lower_paragraph() {
    let input = lines_vec!("1. One", "", "Paragraph", "3. Next");
    let expected = lines_vec!("1. One", "", "Paragraph", "1. Next");
    assert_eq!(renumber_lists(&input), expected);
}

#[test]
fn no_restart_without_blank() {
    let input = lines_vec!("1. One", "Paragraph", "3. Next");
    let expected = lines_vec!("1. One", "Paragraph", "2. Next");
    assert_eq!(renumber_lists(&input), expected);
}

#[test]
fn no_restart_for_indented_paragraph() {
    let input = lines_vec!("1. One", "", "  Indented", "3. Next");
    let expected = lines_vec!("1. One", "", "  Indented", "2. Next");
    assert_eq!(renumber_lists(&input), expected);
}

#[test]
fn no_restart_for_non_plain_line() {
    let input = lines_vec!("1. One", "", "# Heading", "3. Next");
    let expected = lines_vec!("1. One", "", "# Heading", "2. Next");
    assert_eq!(renumber_lists(&input), expected);
}

#[test]
fn restart_after_nested_paragraph() {
    let input = lines_vec!("1. One", "    1. Sub", "", "Paragraph", "3. Next");
    let expected = lines_vec!("1. One", "    1. Sub", "", "Paragraph", "1. Next");
    assert_eq!(renumber_lists(&input), expected);
}
