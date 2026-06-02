//! Stateful properties for link reference continuation windows.

use proptest::prelude::*;

use crate::wrap::link_reference::{LinkReferenceMatcher, LinkTitleWindow, LinkTitleWindowOutcome};

#[derive(Debug, Clone, Copy)]
enum Action {
    OpenBareDefinition,
    OpenBareLabel,
    Fence,
    Blank,
    Title,
    Url,
    UrlWithInlineTitle,
    MarkdownBlock,
    Prose,
}

fn action_strategy() -> impl Strategy<Value = Action> {
    prop_oneof![
        Just(Action::OpenBareDefinition),
        Just(Action::OpenBareLabel),
        Just(Action::Fence),
        Just(Action::Blank),
        Just(Action::Title),
        Just(Action::Url),
        Just(Action::UrlWithInlineTitle),
        Just(Action::MarkdownBlock),
        Just(Action::Prose),
    ]
}

fn observe_line_for(action: Action) -> Option<&'static str> {
    match action {
        Action::Blank => Some(""),
        Action::Title => Some("  \"Title\""),
        Action::Url => Some("  https://example.com"),
        Action::UrlWithInlineTitle => Some("  https://example.com \"Title\""),
        Action::MarkdownBlock => Some(" - list item"),
        Action::Prose => Some("plain prose"),
        Action::OpenBareDefinition | Action::OpenBareLabel | Action::Fence => None,
    }
}

proptest! {
    #[test]
    fn link_title_window_sequences_preserve_terminal_states(
        actions in prop::collection::vec(action_strategy(), 0..40),
    ) {
        let matcher = LinkReferenceMatcher::production();
        let mut window = LinkTitleWindow::Closed;

        for action in actions {
            match action {
                Action::OpenBareDefinition => {
                    window.observe_bare_definition();
                    prop_assert_eq!(window, LinkTitleWindow::AwaitingStandaloneTitle);
                }
                Action::OpenBareLabel => {
                    window.observe_bare_label();
                    prop_assert_eq!(window, LinkTitleWindow::AwaitingUrlContinuation);
                }
                Action::Fence => {
                    window.observe_fence_context();
                    prop_assert_eq!(window, LinkTitleWindow::Closed);
                }
                _ => {
                    let prior = window;
                    let Some(line) = observe_line_for(action) else {
                        continue;
                    };
                    let outcome = window.observe_next_line(line, matcher);

                    if prior == LinkTitleWindow::Closed {
                        prop_assert_eq!(outcome, None);
                        prop_assert_eq!(window, LinkTitleWindow::Closed);
                    }
                    if outcome == Some(LinkTitleWindowOutcome::Reprocess) {
                        prop_assert_eq!(window, LinkTitleWindow::Closed);
                    }
                }
            }
        }
    }
}
