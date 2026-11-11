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
  cut. Both fields are gone. The event now carries `token_length`, the
  character count of the token, and `kind`, the computed fragment
  classification.
- **Who is affected:** Any host application that installs a `tracing`
  subscriber and parses, matches, or indexes the `fragment classified` event by
  field name. Hosts that install no subscriber see no behavioural change, and
  rendered output is unchanged.
- **Migration action:** Read `token_length` and `kind` instead of `token` and
  `truncated`. No replacement snippet of document content is available at any
  level: the boundary deliberately keeps document text out of tracing so
  diagnostics cannot disclose credentials or personal data. A consumer that
  needs the matched text must derive it from the document it supplied to the
  wrapper rather than from an event field.

The event's level, message, and every other field are unchanged, so a
subscriber that does not key on `token` or `truncated` needs no change.
