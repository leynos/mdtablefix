# Footnote Conversion

`mdtablefix` can optionally convert bare numeric references into
GitHub-flavoured Markdown footnotes. The `convert_footnotes` function performs
this operation and is exposed via the higher-level `process_stream_opts` helper.

Inline references that appear after punctuation are rewritten as footnote links.

Before:

```markdown
A useful tip.1
```

After:

```markdown
A useful tip.[^1]
```

Numbers inside inline code or parentheses are ignored.

Before:

```markdown
Look at `code 1` for details.
Refer to equation (1) for context.
```

After:

```markdown
Look at `code 1` for details.
Refer to equation (1) for context.
```

When the final lines of a document form a numbered list, they are replaced with
footnote definitions.

Before:

```markdown
Text.

 1. First note
 2. Second note
```

After:

```markdown
Text.

 [^1] First note
[^2] Second note
```

`convert_footnotes` only processes the final contiguous list of numeric
references.
