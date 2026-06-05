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

fn terminal_line_after_bare_label_strategy()
-> impl Strategy<Value = (&'static str, LinkTitleWindowOutcome)> {
    prop::sample::select(vec![
        ("", LinkTitleWindowOutcome::EmitVerbatim),
        (
            "  https://example.com \"Title\"",
            LinkTitleWindowOutcome::EmitVerbatim,
        ),
        ("plain prose", LinkTitleWindowOutcome::Reprocess),
        (" - list item", LinkTitleWindowOutcome::Reprocess),
    ])
}

fn title_window_line_strategy() -> impl Strategy<Value = (&'static str, LinkTitleWindowOutcome)> {
    prop::sample::select(vec![
        ("", LinkTitleWindowOutcome::EmitVerbatim),
        ("  \"Title\"", LinkTitleWindowOutcome::EmitVerbatim),
        ("plain prose", LinkTitleWindowOutcome::Reprocess),
        (" - list item", LinkTitleWindowOutcome::Reprocess),
    ])
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

    #[test]
    fn bare_label_terminal_continuations_close_window(
        (line, expected) in terminal_line_after_bare_label_strategy(),
    ) {
        let matcher = LinkReferenceMatcher::production();
        let mut window = LinkTitleWindow::Closed;

        window.observe_bare_label();
        prop_assert_eq!(window.observe_next_line(line, matcher), Some(expected));
        prop_assert_eq!(window, LinkTitleWindow::Closed);
    }

    #[test]
    fn bare_label_url_then_title_window_closes_on_second_line(
        (second_line, expected) in title_window_line_strategy(),
    ) {
        let matcher = LinkReferenceMatcher::production();
        let mut window = LinkTitleWindow::Closed;

        window.observe_bare_label();
        prop_assert_eq!(
            window.observe_next_line("  https://example.com", matcher),
            Some(LinkTitleWindowOutcome::EmitVerbatim)
        );
        prop_assert_eq!(window, LinkTitleWindow::AwaitingStandaloneTitle);

        prop_assert_eq!(
            window.observe_next_line(second_line, matcher),
            Some(expected)
        );
        prop_assert_eq!(window, LinkTitleWindow::Closed);
    }
}
