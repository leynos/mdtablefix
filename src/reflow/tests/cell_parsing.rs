//! Tests for `Cell` parsing and parse-stage cleanup.

use super::*;

#[test]
fn parse_cells_marks_leading_empty_cells_without_reparsing() {
    let cells = parse_cells("|   | keep \\| literal | tail |");

    assert_eq!(
        normalize_cells(std::slice::from_ref(&cells)),
        vec![vec![
            String::new(),
            "keep | literal".to_owned(),
            "tail".to_owned(),
        ]]
    );
    assert!(cells[0].leading_empty);
}

#[test]
fn parse_cells_preserves_adjacent_interior_empty_cell() {
    let cells = parse_cells("| | ROW_END || ROW_END |");

    assert_eq!(
        normalize_cells(std::slice::from_ref(&cells)),
        vec![vec![
            String::new(),
            "ROW_END".to_owned(),
            String::new(),
            "ROW_END".to_owned(),
        ]]
    );
    assert!(cells[0].leading_empty);
    assert!(!cells[2].leading_empty);
}

#[test]
fn parse_cells_leaves_non_continuation_rows_unmarked() {
    let line = "| head | body \\| value |";

    assert_eq!(
        normalize_cells(&[parse_cells(line)]),
        vec![vec!["head".to_owned(), "body | value".to_owned()]]
    );
}

#[test]
fn clean_rows_restores_leading_empty_cells_and_discards_empty_rows() {
    let rows = vec![
        vec![
            Cell {
                payload: String::new(),
                leading_empty: true,
            },
            Cell {
                payload: "value".to_owned(),
                leading_empty: false,
            },
        ],
        vec![
            Cell {
                payload: String::new(),
                leading_empty: false,
            },
            Cell {
                payload: String::new(),
                leading_empty: false,
            },
        ],
    ];

    assert_eq!(
        clean_rows(rows),
        vec![vec![String::new(), "value".to_owned()]]
    );
}
