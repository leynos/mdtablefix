# Module Relationships

This diagram illustrates the connections between the crate's modules.

```mermaid
classDiagram
    class lib {
        <<module>>
    }
    class html {
        <<module>>
        +convert_html_tables()
        +html_table_to_markdown()
    }
    class table {
        <<module>>
        +reflow_table()
        +split_cells()
        +SEP_RE
    }
    class wrap {
        <<module>>
        +wrap_text()
        +is_fence()
    }
    class lists {
        <<module>>
        +renumber_lists()
    }
    class breaks {
        <<module>>
        +format_breaks()
        +THEMATIC_BREAK_LEN
    }
    class process {
        <<module>>
        +process_stream()
        +process_stream_no_wrap()
    }
    class io {
        <<module>>
        +rewrite()
        +rewrite_no_wrap()
    }
    lib --> html
    lib --> table
    lib --> wrap
    lib --> lists
    lib --> breaks
    lib --> process
    lib --> io
    html ..> wrap : uses is_fence
    table ..> reflow : uses parse_rows, etc.
    lists ..> wrap : uses is_fence
    breaks ..> wrap : uses is_fence
    process ..> html : uses convert_html_tables
    process ..> table : uses reflow_table
    process ..> wrap : uses wrap_text, is_fence
    io ..> process : uses process_stream, process_stream_no_wrap
```

The `lib` module re-exports the public API from the other modules. The
`process` module provides streaming helpers that combine the lower-level
functions. The `io` module handles filesystem operations, delegating the text
processing to `process`.
