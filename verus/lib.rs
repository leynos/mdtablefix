//! Root entry point for production-used Verus kernels.
//!
//! Each verified kernel must include the production module it proves with a
//! `#[path]` attribute, or otherwise carry a refinement proof to the formatter
//! function that calls it. Do not add a standalone reimplementation here.

use vstd::prelude::*;

verus! {

#[path = "../src/classify_kernel.rs"]
pub mod production_classify;

/// The production fence transition kernel, included so the specification in
/// `verus/fence_spec.rs` constrains the body the formatter actually runs.
///
/// The kernel is included rather than reimplemented for the same reason the
/// classifier is: a parallel model would be free to drift, and a proof about a
/// model that is not the production code proves nothing about the formatter.
#[path = "../src/wrap/fence/kernel.rs"]
pub mod production_fence;

/// The production compression planner, included for the same reason as the
/// kernel above.
///
/// The planner is the whole of the compression pass's decision-making: which
/// block is open, whether it may be compressed, which positions of a released
/// block are rewritten, and what the agenda is. The pass keeps only the parsing
/// and the text rendering, so proving this module is proving the pass's
/// behaviour rather than a model of it.
#[path = "../src/fences/plan.rs"]
pub mod production_plan;

use production_classify::LineClass;

/// State relevant to a structural decision after source scanning.
pub struct ClassifyCtxView {
    pub is_in_fence: bool,
    pub open_fence: Option<production_classify::OpenFence>,
    pub previous: Option<LineClass>,
    pub prefix_agrees: bool,
}

impl View for production_classify::ClassifyCtxKernel {
    type V = ClassifyCtxView;

    open spec fn view(&self) -> ClassifyCtxView {
        ClassifyCtxView {
            is_in_fence: self.is_in_fence,
            open_fence: self.open_fence,
            previous: self.previous,
            prefix_agrees: self.prefix_agrees,
        }
    }
}

#[path = "classify_spec.rs"]
pub mod classify_spec;
pub use classify_spec::*;

#[path = "setext_spec.rs"]
pub mod setext_spec;

#[path = "fence_spec.rs"]
pub mod fence_spec;
pub use fence_spec::*;

#[path = "plan_spec.rs"]
pub mod plan_spec;
// The planner names its specification functions through the crate root, because
// `src/fences/plan.rs` is compiled both as a production module and as an
// included proof module, and the second form has only this file's exports to
// reach them through.
pub use plan_spec::*;

/// The fence kernel's view is itself.
///
/// Every field is a plain `Copy` value — a depth, an optional marker character,
/// a run length, and a trailing-blankness flag — so the mathematical form of a
/// [`LineFeatures`] is the struct itself: the specification reasons about the
/// same values the kernel carries, with no abstraction that could disagree.
impl View for production_fence::LineFeatures {
    type V = production_fence::LineFeatures;

    open spec fn view(&self) -> production_fence::LineFeatures {
        *self
    }
}

/// A fence opener's view is likewise itself.
impl View for production_fence::FenceState {
    type V = production_fence::FenceState;

    open spec fn view(&self) -> production_fence::FenceState {
        *self
    }
}

/// A region's view is itself.
impl View for production_fence::Region {
    type V = production_fence::Region;

    open spec fn view(&self) -> production_fence::Region {
        *self
    }
}

/// The planner's buffering state views as itself.
///
/// `Block` holds a [`FenceState`] and a `bool`, and both of those already view
/// as themselves, so the record needs no projection: the specification reads the
/// same opener and the same flag the planner carries, and a specification clause
/// can name `block.opening` directly rather than destructuring a pair.
impl View for production_plan::Block {
    type V = production_plan::Block;

    open spec fn view(&self) -> production_plan::Block {
        *self
    }
}

/// A release's view is itself: three names for three ways a block ends.
impl View for production_plan::Release {
    type V = production_plan::Release;

    open spec fn view(&self) -> production_plan::Release {
        *self
    }
}

/// An agendum's view is itself: three names for three ways a line is written.
impl View for production_plan::Agendum {
    type V = production_plan::Agendum;

    open spec fn view(&self) -> production_plan::Agendum {
        *self
    }
}

/// A step's view is itself. Each variant carries planner values whose own views
/// are stated above, so a step needs no projection of its own.
impl View for production_plan::Step {
    type V = production_plan::Step;

    open spec fn view(&self) -> production_plan::Step {
        *self
    }
}

/// What the caller learned about a line views as the planner's own input.
///
/// The features and the rewrite flag are the two grammar-dependent facts the
/// caller supplies, and the two routing answers are the tracker's. None of them
/// is derived, so the view is the record itself.
impl View for production_plan::LineFacts {
    type V = production_plan::LineFacts;

    open spec fn view(&self) -> production_plan::LineFacts {
        *self
    }
}

/// The driver's buffering state views as itself.
///
/// Both fields are plain `Copy` values the planner chose on lines it has already
/// buffered, so the specification reads the same block, the same starting index
/// and the same rewrite flag the kernel carries, with no projection that could
/// disagree.
impl View for production_plan::DriverState {
    type V = plan_spec::DriveState;

    open spec fn view(&self) -> plan_spec::DriveState {
        plan_spec::DriveState {
            pending: match self.pending {
                Some(block) => Some(block),
                None => None,
            },
            start: self.start as int,
            opener_rewritable: self.opener_rewritable,
        }
    }
}

/// Structural precedence over the exact body and context supplied to the kernel.
pub open spec fn spec_open_text(s: Seq<char>, start: int, ctx: ClassifyCtxView) -> LineClass {
    if spec_fence_marker(s, start) {
        LineClass::FenceMarker
    } else if spec_atx_heading(s, start) {
        LineClass::AtxHeading
    } else if spec_table_delimiter(s, start) {
        LineClass::TableDelimiter
    } else if spec_body_starts_with_pipe(s, start) {
        LineClass::TableRow
    } else if matches!(ctx.previous, Some(LineClass::ParagraphText))
        && ctx.prefix_agrees && spec_setext_underline(s, start) {
        LineClass::SetextUnderline
    } else if spec_thematic_break(s, start) {
        LineClass::ThematicBreak
    } else if spec_list_item(s, start) {
        LineClass::ListItem
    } else {
        LineClass::ParagraphText
    }
}

pub open spec fn spec_within_fence(s: Seq<char>, start: int, ctx: ClassifyCtxView) -> LineClass {
    if matches!(ctx.open_fence, Some(open) if spec_closing_fence(s, start, open)) {
        LineClass::FenceMarker
    } else {
        LineClass::Literal
    }
}

/// Structural precedence over the exact body and context supplied to the kernel.
pub open spec fn spec_classify(s: Seq<char>, ctx: ClassifyCtxView) -> LineClass {
    let (body_start, is_literal) = spec_line_parts(s);
    if ctx.is_in_fence {
        if is_literal {
            LineClass::Literal
        } else {
            spec_within_fence(s, body_start, ctx)
        }
    } else if spec_is_blank_from(s, 0) || spec_is_blank_from(s, body_start) {
        LineClass::Blank
    } else if is_literal {
        LineClass::Literal
    } else {
        spec_open_text(s, body_start, ctx)
    }
}

pub open spec fn canonical_context() -> ClassifyCtxView {
    ClassifyCtxView { is_in_fence: false, open_fence: None, previous: None, prefix_agrees: false }
}

pub open spec fn canonical_break() -> Seq<char> { Seq::<char>::new(70, |i: int| '_') }

/// Non-vacuity witnesses for the classifier above, and their shared step.
///
/// The classifier's own definitions stay in this file because the
/// production modules reach them through the crate root; only the evidence
/// that they are not vacuous moves out. Declaring the witnesses here rather
/// than in this file's body keeps the specification readable as a statement
/// and brings this file under the 400-line ceiling `AGENTS.md` sets.
///
/// The child module needs `spec_classify`, `canonical_break` and
/// `canonical_context`, so it is declared after all three are defined.
#[path = "classify_spec_witnesses.rs"]
mod classify_witnesses;

} // verus!

fn main() {}
