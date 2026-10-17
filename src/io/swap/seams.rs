//! Test-only, thread-local seams for deterministic atomic-swap failure tests.

use super::{Dir, Utf8Path};

#[cfg(test)]
pub(crate) mod rename_failure_seam {
    //! A test-only seam that fails the rename half of the swap.
    //!
    //! Every rename a test can be made to fail for real fails before the
    //! destination is prepared, so the rollback in `swap_into_place` is
    //! otherwise unreachable. The arming is per-thread, because the tests that
    //! use it drive the swap on the thread that armed it, and it is undone when
    //! the value [`arm`] returns is dropped, so a failing assertion cannot
    //! leave the failure armed for whatever runs next on that thread.

    use std::cell::Cell;

    thread_local! {
        /// Whether this thread's next rename must fail.
        static ARMED: Cell<bool> = const { Cell::new(false) };
    }

    /// Arms the seam until the returned value is dropped.
    pub(crate) fn arm() -> Armed {
        ARMED.with(|armed| armed.set(true));
        Armed
    }

    /// Disarms the seam when dropped.
    pub(crate) struct Armed;

    impl Drop for Armed {
        fn drop(&mut self) { ARMED.with(|armed| armed.set(false)); }
    }

    /// Consumes the arming, reporting whether this rename must fail.
    ///
    /// One-shot by design: arming fails exactly one swap, so a test that
    /// triggers more than one rename cannot have the seam fire twice.
    pub(crate) fn take() -> bool { ARMED.with(|armed| armed.replace(false)) }
}

#[cfg(test)]
pub(crate) mod cleanup_failure_seam {
    //! A test-only seam that fails the removal of a temporary file.
    //!
    //! The cleanup after a failed replacement is otherwise reachable only
    //! through a real removal failure, which no test can force on every
    //! platform: a directory target fails the swap before a temporary file is
    //! named, and a permission bit is ignored by a run as root. The arming is
    //! per-thread, because the tests that use it drive the replacement on the
    //! thread that armed it, and it is undone when the value [`arm`] returns is
    //! dropped, so a failing assertion cannot leave the removal failing for
    //! whatever runs next on that thread.

    use std::cell::Cell;

    thread_local! {
        /// Whether this thread's next removal must fail.
        static ARMED: Cell<bool> = const { Cell::new(false) };
    }

    /// Arms the seam until the returned value is dropped.
    pub(crate) fn arm() -> Armed {
        ARMED.with(|armed| armed.set(true));
        Armed
    }

    /// Disarms the seam when dropped.
    pub(crate) struct Armed;

    impl Drop for Armed {
        fn drop(&mut self) { ARMED.with(|armed| armed.set(false)); }
    }

    /// Consumes the arming, reporting whether this removal must fail.
    ///
    /// One-shot by design: arming fails exactly one removal, so a test that
    /// triggers more than one cleanup cannot have the seam fire twice.
    pub(crate) fn take() -> bool { ARMED.with(|armed| armed.replace(false)) }
}

#[cfg(test)]
pub(crate) mod competing_writer_seam {
    //! A test-only seam that lets a case land another writer inside the swap.
    //!
    //! The window it opens is the one no platform lets the swap close: no
    //! rename compares contents, so a writer that lands between the swap's last
    //! comparison and its rename is overwritten by it. Without the seam a test
    //! can only reach that window by racing the scheduler, and a case that wins
    //! the race on one machine loses it on the next. Here the write is handed
    //! the directory capability and the target the swap is working with, so it
    //! lands in the window deterministically and through the same capability as
    //! every other operation.
    //!
    //! The arming is per-thread, because the tests that use it drive the swap on
    //! the thread that armed it, and it is undone when the value [`arm`] returns
    //! is dropped, so a failing assertion cannot leave an intrusion armed for
    //! whatever runs next on that thread.

    use std::cell::Cell;

    use super::{Dir, Utf8Path};

    /// The write an armed seam runs before the swap's final comparison.
    ///
    /// Boxed so the arming can hold one concrete closure of any shape, and
    /// aliased so the thread-local below states what it holds rather than the
    /// shape of a boxed trait object.
    type Intrusion = Box<dyn FnOnce(&Dir, &Utf8Path)>;

    thread_local! {
        /// The write this thread's next swap must run before its final comparison.
        static ARMED: Cell<Option<Intrusion>> = const { Cell::new(None) };
    }

    /// Arms the seam with the write to run inside the swap, until the returned
    /// value is dropped.
    ///
    /// The write is given the capability and the target as arguments, so the
    /// arming captures no path and no handle of its own.
    pub(crate) fn arm(intrude: impl FnOnce(&Dir, &Utf8Path) + 'static) -> Armed {
        ARMED.with(|armed| armed.set(Some(Box::new(intrude))));
        Armed
    }

    /// Disarms the seam when dropped.
    pub(crate) struct Armed;

    impl Drop for Armed {
        fn drop(&mut self) { ARMED.with(|armed| armed.set(None)); }
    }

    /// Runs the armed intrusion, if any, exactly once.
    ///
    /// One-shot by design: an intrusion arms exactly one swap, so a test that
    /// triggers more than one swap cannot have the seam fire twice.
    pub(crate) fn run(directory: &Dir, path: &Utf8Path) {
        if let Some(intrude) = ARMED.with(Cell::take) {
            intrude(directory, path);
        }
    }
}
