//! The metrics the wrapping path emits.
//!
//! Wrapping is the tool's hot path: every analysed file's prose passes through
//! [`wrap_text`](super::wrap_text). These metrics make that work visible from
//! the composition boundary rather than from inside the domain modules, which
//! is the same separation the observer port keeps for diagnostics.
//!
//! Every name and label value is fixed, so a recorder's cardinality stays
//! bounded: no path, file name, or document text is ever a label. The library
//! emits these metrics but never installs a recorder; a host application
//! installs one, as documented in the developers' guide.

use std::time::Duration;

use metrics::{Unit, counter, describe_counter, describe_histogram, histogram};

/// The counter recording wrapping invocations.
pub(super) const WRAP_TOTAL: &str = "mdtablefix_wrap_total";

/// The histogram recording each wrapping invocation's duration.
pub(super) const WRAP_DURATION: &str = "mdtablefix_wrap_duration_seconds";

/// The `outcome` label value for an invocation that changed the text.
pub(super) const REFLOWED: &str = "reflowed";

/// The `outcome` label value for an invocation that left the text alone.
pub(super) const UNCHANGED: &str = "unchanged";

/// Registers the descriptions of every metric the wrapping path emits.
///
/// Registration is repeated on each invocation rather than cached. A
/// description reaches only the recorder that is active at that moment, so a
/// process-wide cache would bind it to whichever recorder happened to be
/// installed during the first wrap — the no-op recorder, or another thread's
/// local one. Two calls through the façade's recorder pointer cost far less
/// than the wrapping work they describe.
pub(super) fn register_metrics() {
    describe_counter!(
        WRAP_TOTAL,
        "Wrapping invocations, by whether any line was reflowed"
    );
    describe_histogram!(
        WRAP_DURATION,
        Unit::Seconds,
        "Duration of one wrapping invocation, by whether any line was reflowed"
    );
}

/// Records one wrapping invocation's outcome and duration.
///
/// `input` and `output` are compared only to choose the bounded `outcome`
/// label; neither is recorded. The duration is recorded for every invocation,
/// so a wrap that stalls is visible rather than missing from the distribution.
/// `elapsed` measures the wrapping work alone: it is read before the comparison
/// below, so label selection and recording are outside the sample.
pub(super) fn record_wrap(input: &[String], output: &[String], elapsed: Duration) {
    register_metrics();
    let outcome = if output == input { UNCHANGED } else { REFLOWED };
    counter!(WRAP_TOTAL, "outcome" => outcome).increment(1);
    histogram!(WRAP_DURATION, "outcome" => outcome).record(elapsed.as_secs_f64());
}
