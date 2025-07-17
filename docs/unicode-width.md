# Unicode Width Handling

`mdtablefix` wraps paragraphs and list items while respecting the display width
of Unicode characters. The `unicode-width` crate is used to compute the width of
strings when deciding where to break lines. This prevents emojis or other
multi-byte characters from causing unexpected wraps or truncation.

Whenever wrapping logic examines the length of a token, it relies on
`UnicodeWidthStr::width` to measure visible columns rather than byte length.
