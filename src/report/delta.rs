//! Line-level insertion and deletion counts.

use similar::{ChangeTag, TextDiff};

/// Counts of lines inserted and deleted between two texts.
///
/// A modified line counts as one insertion and one deletion, matching
/// `git diff --numstat`.
///
/// # Examples
///
/// ```
/// use mdtablefix::report::LineDelta;
///
/// let delta = LineDelta::between("alpha\n", "alpha\nbeta\n");
/// assert_eq!(delta.insertions(), 1);
/// assert_eq!(delta.deletions(), 0);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LineDelta {
    insertions: usize,
    deletions: usize,
}

impl LineDelta {
    /// Counts the line-level changes turning `original` into `formatted`.
    ///
    /// Callers must not invoke this when the texts are byte-equal; the caller
    /// compares bytes first so a clean tree costs no diff work.
    ///
    /// The count comes from `similar`'s line tokenizer rather than from
    /// [`str::lines`], so it classifies exactly the lines the unified diff
    /// renders. The two disagree only on a lone carriage return, which the
    /// tokenizer treats as a terminator and `str::lines` does not.
    ///
    /// # Examples
    ///
    /// ```
    /// use mdtablefix::report::LineDelta;
    ///
    /// let delta = LineDelta::between("|A|B|\n", "| A | B |\n");
    /// assert_eq!((delta.insertions(), delta.deletions()), (1, 1));
    /// ```
    #[must_use]
    pub fn between(original: &str, formatted: &str) -> Self {
        let mut delta = Self::default();
        for change in TextDiff::from_lines(original, formatted).iter_all_changes() {
            match change.tag() {
                ChangeTag::Insert => delta.insertions += 1,
                ChangeTag::Delete => delta.deletions += 1,
                ChangeTag::Equal => {}
            }
        }
        delta
    }

    /// The number of lines that would be added.
    ///
    /// # Examples
    ///
    /// ```
    /// use mdtablefix::report::LineDelta;
    ///
    /// assert_eq!(LineDelta::between("", "alpha\n").insertions(), 1);
    /// ```
    #[must_use]
    pub const fn insertions(self) -> usize { self.insertions }

    /// The number of lines that would be removed.
    ///
    /// # Examples
    ///
    /// ```
    /// use mdtablefix::report::LineDelta;
    ///
    /// assert_eq!(LineDelta::between("alpha\n", "").deletions(), 1);
    /// ```
    #[must_use]
    pub const fn deletions(self) -> usize { self.deletions }

    /// Whether either count is non-zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use mdtablefix::report::LineDelta;
    ///
    /// assert!(LineDelta::between("alpha\n", "beta\n").has_changes());
    /// assert!(!LineDelta::between("alpha\n", "alpha\n").has_changes());
    /// ```
    #[must_use]
    pub const fn has_changes(self) -> bool { self.insertions > 0 || self.deletions > 0 }
}

#[cfg(test)]
mod tests {
    //! Unit tests for line counting.

    use super::LineDelta;

    #[test]
    fn pure_insertion_counts_only_insertions() {
        let delta = LineDelta::between("alpha\n", "alpha\nbeta\n");
        assert_eq!((delta.insertions(), delta.deletions()), (1, 0));
        assert!(delta.has_changes(), "an insertion alone is a change");
    }

    #[test]
    fn pure_deletion_counts_only_deletions() {
        let delta = LineDelta::between("alpha\nbeta\n", "alpha\n");
        assert_eq!((delta.insertions(), delta.deletions()), (0, 1));
        assert!(delta.has_changes(), "a deletion alone is a change");
    }

    #[test]
    fn replacement_counts_both_sides() {
        let delta = LineDelta::between("alpha\n", "beta\n");
        assert_eq!((delta.insertions(), delta.deletions()), (1, 1));
    }

    #[test]
    fn identical_texts_have_no_changes() {
        let delta = LineDelta::between("alpha\nbeta\n", "alpha\nbeta\n");
        assert_eq!((delta.insertions(), delta.deletions()), (0, 0));
        assert!(!delta.has_changes());
    }

    /// Rewriting a document's endings replaces every line, because the
    /// tokenizer treats `\r\n` and `\n` as different terminators. That is what
    /// `git diff --numstat` reports too. A file whose endings are already
    /// uniform never reaches here: the caller compares bytes first.
    #[test]
    fn a_line_ending_change_is_a_full_line_change() {
        let delta = LineDelta::between("alpha\n", "alpha\r\n");
        assert_eq!((delta.insertions(), delta.deletions()), (1, 1));
    }

    /// A lone carriage return is a terminator to the tokenizer, so the counts
    /// follow the lines the unified diff would render rather than the lines
    /// [`str::lines`] would yield.
    #[test]
    fn a_lone_carriage_return_separates_lines() {
        let delta = LineDelta::between("alpha\rbeta\n", "alpha\rgamma\n");
        assert_eq!((delta.insertions(), delta.deletions()), (1, 1));
    }
}
