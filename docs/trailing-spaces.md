# Trailing spaces

`wrap_preserving_code` keeps trailing spaces on the final line.

Markdown treats two spaces at the end of a line as a hard break. Earlier
versions trimmed those spaces during the final flush, turning hard breaks into
soft ones. The final line is now pushed as-is so trailing whitespace survives
wrapping.

## Example

Before:

```rust
assert_eq!(
    wrap_preserving_code("ends with space  ", 80),
    vec!["ends with space"]
);
```

After:

```rust
assert_eq!(
    wrap_preserving_code("ends with space  ", 80),
    vec!["ends with space  "]
);
```

See [issue #65](https://github.com/leynos/mdtablefix/issues/65) for more
information.
