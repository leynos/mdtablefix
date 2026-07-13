# Protect literal regions during ellipsis replacement

## Status

Accepted.

## Context

The `--ellipsis` transform originally normalized every three-dot run in a
plain-text tokenizer token. Markdown links, autolinks, bare URLs, and
filesystem paths are represented as text by that tokenizer even though dots in
those regions may carry syntax or identify a literal resource. GitHub compare
URLs, for example, use `...` between revisions.

Promoting URL and path recognition into the shared tokenizer would make a
typography-specific policy part of its public token model. It would also risk a
breaking change for consumers that exhaustively match the public `Token` enum.

## Decision

Keep literal-region policy local to the ellipsis transform. The transform
reuses the wrapping parser's balanced link and image span detection, but owns
autolink, bare URL, and filesystem-token classification itself. Protected
source ranges are merged and copied byte-for-byte; ellipsis normalization is
applied only to gaps between those ranges.

Treat whitespace-delimited tokens containing `...` as filesystem-like when they
use an absolute, relative, home-relative, or Windows drive prefix, or contain a
path separator. This deliberately conservative heuristic favours preserving a
possibly semantic token over applying typography inside it.

Preserve complete inline links and images, including their labels and optional
titles. This keeps a Markdown construct atomic and avoids partially rewriting
text whose exact bytes may be used by documentation tooling.

## Consequences

- GitHub compare URLs and path examples remain valid under `--ellipsis`.
- Ordinary prose surrounding protected tokens continues to be normalized.
- Some path-shaped prose, such as a slash-separated word containing `...`, is
  intentionally left unchanged.
- Transforms outwith ellipsis replacement do not inherit this policy
  automatically.
