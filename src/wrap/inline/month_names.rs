//! Canonical English month-name tokens for date-like inline grouping.
//!
//! The date predicates and property-test strategies share this list so the
//! generated test inputs stay aligned with the production matcher. It contains
//! twelve full names plus eleven three-letter abbreviations because `May` is
//! identical in both forms.

/// Full and abbreviated English month names recognised in prose dates.
///
/// The entries are grouped by byte length so `is_month_name` can avoid
/// scanning impossible candidates.
pub(crate) const MONTH_NAMES: [&str; 23] = [
    "May",
    "Jan",
    "Feb",
    "Mar",
    "Apr",
    "Jun",
    "Jul",
    "Aug",
    "Sep",
    "Oct",
    "Nov",
    "Dec",
    "June",
    "July",
    "March",
    "April",
    "August",
    "January",
    "October",
    "February",
    "November",
    "December",
    "September",
];
