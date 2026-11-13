# Migrating to 0.7.0

This note is for consumers of the `mdtablefix` library who read the structured
`tracing` events the inline wrapper emits. It covers the behaviour change in
the 0.7.0 release and the migration action required for it
([#309](https://github.com/leynos/mdtablefix/issues/309)).

## Content-free `fragment classified` event

- **What changed:** The inline wrapper no longer builds `tracing` events in its
  tokenizing and classification modules. Those modules emit borrowed domain
  events through an injected observer, and the adapter in
  `src/wrap/tracing_adapter.rs` translates them into the same `tracing` output.
  As part of that boundary the `fragment classified` event stopped carrying
  document text. Its `token` field held a truncated, up-to-80-byte snippet of
  the source, and its `truncated` flag reported whether the snippet had been
  cut. Both fields are gone. A new `token_length` field gives the token's
  character count. The `kind` field is unchanged: it already reported the
  computed fragment classification and still does.
- **Who is affected:** Any host application that installs a `tracing`
  subscriber and parses, matches, or indexes the `fragment classified` event by
  field name. Hosts that install no subscriber see no behavioural change, and
  rendered output is unchanged.
- **Migration action:** A consumer that read `token` must switch to
  `token_length`, which is a character count and not a replacement snippet.
  There is no replacement for `truncated`: the flag described whether the old
  snippet had been cut, and no snippet remains to cut. A consumer that keyed on
  `kind` needs no change. No replacement snippet of document content is
  available at any level: the boundary deliberately keeps document text out of
  tracing so diagnostics cannot disclose credentials or personal data. A
  consumer that needs the matched text must derive it from the document it
  supplied to the wrapper rather than from an event field.

The event's level, message, and every other field are unchanged, so a
subscriber that does not key on `token` or `truncated` needs no change.
