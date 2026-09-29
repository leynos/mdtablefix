//! Holds runner placement: where each lane runs, and for how long.
//!
//! Ubicloud's cache proxy is scoped by ref and a fork's pull request cannot
//! obtain an Ubicloud runner, so a lane that names one selects it by an
//! expression that falls back to the hosted pool and states its own ceiling.
//! The judgement lives in [`placement`] and is driven against constructed
//! expressions and workflows in [`placement_cases`] before it is applied to the
//! real files, because a check run only over this repository's own correct
//! workflows would pass whether or not it detected anything.
//!
//! The CV-005 `CodeScene` contract is not here: `make test-workflow-contracts`
//! runs it from the shared `cv005-contracts` library.

#[path = "runner_placement/placement.rs"]
mod placement;
#[path = "runner_placement/placement_cases.rs"]
mod placement_cases;
#[path = "runner_placement/reader.rs"]
mod reader;
