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

    use rstest::rstest;

    use super::LineDelta;

    /// Each pair of texts, the counts it produces, and whether it is a change.
    ///
    /// The flag is asserted beside the counts rather than derived from them:
    /// a pure insertion and a pure deletion each already produce the counts
    /// that a wrong comparison in [`LineDelta::has_changes`] would satisfy, so
    /// this column is what rejects it. The cases are named rather than
    /// positional so a failure says which shape it was.
    #[rstest]
    #[case::pure_insertion("alpha\n", "alpha\nbeta\n", 1, 0, true)]
    #[case::pure_deletion("alpha\nbeta\n", "alpha\n", 0, 1, true)]
    #[case::replacement("alpha\n", "beta\n", 1, 1, true)]
    #[case::identical("alpha\nbeta\n", "alpha\nbeta\n", 0, 0, false)]
    // Rewriting a document's endings replaces every line, because the
    // tokenizer treats `\r\n` and `\n` as different terminators. That is what
    // `git diff --numstat` reports too. A file whose endings are already
    // uniform never reaches here: the caller compares bytes first.
    #[case::line_ending_change("alpha\n", "alpha\r\n", 1, 1, true)]
    // A lone carriage return is a terminator to the tokenizer, so the counts
    // follow the lines the unified diff would render rather than the lines
    // `str::lines` would yield.
    #[case::lone_carriage_return("alpha\rbeta\n", "alpha\rgamma\n", 1, 1, true)]
    fn between_counts_lines(
        #[case] original: &str,
        #[case] formatted: &str,
        #[case] insertions: usize,
        #[case] deletions: usize,
        #[case] changes: bool,
    ) {
        let delta = LineDelta::between(original, formatted);

        assert_eq!(
            (delta.insertions(), delta.deletions()),
            (insertions, deletions)
        );
        assert_eq!(delta.has_changes(), changes);
    }
}
