# HTML Table Support in `mdtablefix`

`mdtablefix` can format simple HTML `<table>` elements embedded in Markdown.
These HTML tables are transformed into Markdown before the main table reflow
logic runs. That preprocessing is handled by the `convert_html_tables` function.

Only straightforward tables with `<tr>`, `<th>` and `<td>` tags are detected.
Attributes and tag casing are ignored, and complex nested or styled tables are
not supported. After conversion each HTML table is represented as a Markdown
table so the usual reflow algorithm can align its columns consistently with the
rest of the document.

```html
<table>
  <tr><th>A</th><th>B</th></tr>
  <tr><td>1</td><td>2</td></tr>
</table>
```
