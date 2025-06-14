# HTML Table Support

`mdtablefix` uses the `html5ever` parser to recognise simple `<table>` elements
embedded in Markdown documents. These tables are converted to Markdown before the
normal reflow logic runs so that Markdown and HTML tables are formatted
consistently.

The crate `markup5ever_rcdom` provides a minimal DOM which `html5ever` populates
and which is traversed to extract rows and cells. Only basic tables containing
`<tr>`, `<th>` and `<td>` elements are supported.
