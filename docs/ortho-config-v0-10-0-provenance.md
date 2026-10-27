# Provenance of the vendored `ortho_config` v0.10.0 guides

This note records where
[the `ortho_config` user's guide](ortho-config-users-guide.md) and
[the v0.10.0 migration guide](ortho-config-v0-10-0-migration-guide.md) came
from, so a future reader can re-sync them and prove the copies have not
drifted. Both files are byte-for-byte copies of their upstream originals; this
note is kept beside them rather than inside them, precisely so they stay
byte-identical.

## Source

| Field          | Value                                                               |
| -------------- | ------------------------------------------------------------------- |
| Repository     | <https://github.com/leynos/ortho-config>                            |
| Tag            | `v0.10.0`                                                           |
| Tag object     | `996e5a1c9f22ace4f55bf5dcc196d2c0524262bf`                          |
| Commit         | `38812c563e3953a490e3efc202609f37e21a0517`                          |
| Commit date    | 2026-10-25T02:33:53Z                                                |
| Commit subject | Prepare the 0.10.0 release and test every documented example (#620) |

The tag is annotated, so `v0.10.0` names the tag object and `v0.10.0^{commit}`
peels it to the commit. The documentation was read from that commit, never from
`main`, `HEAD`, or a latest-release URL, because those move.

## Imported files

| Local path                                     | Upstream path                     | SHA-256                                                            | Lines |
| ---------------------------------------------- | --------------------------------- | ------------------------------------------------------------------ | ----- |
| `docs/ortho-config-users-guide.md`             | `docs/users-guide.md`             | `84d1c97b77024224dc9220a12d4b749205d9bb2db54849185da21c969dda5575` | 1977  |
| `docs/ortho-config-v0-10-0-migration-guide.md` | `docs/v0-10-0-migration-guide.md` | `b5331aa6cfcc07b3dcd188c620db98d0a1a1f13b0c8ef96d88913d701bca25bf` | 498   |

Upstream's own name for the migration guide carries a `v0-` prefix; this
repository's copy carries the same prefix so the two names stay recognisably
related.

## Re-syncing

A re-sync is a plain diff, because the local copies carry no added text. To
check the copies against the recorded commit:

```console
$ git -C /path/to/ortho-config rev-parse "v0.10.0^{commit}"
38812c563e3953a490e3efc202609f37e21a0517
$ git -C /path/to/ortho-config show \
    "v0.10.0^{commit}:docs/users-guide.md" | sha256sum
84d1c97b77024224dc9220a12d4b749205d9bb2db54849185da21c969dda5575  -
$ sha256sum docs/ortho-config-users-guide.md
84d1c97b77024224dc9220a12d4b749205d9bb2db54849185da21c969dda5575  docs/ortho-config-users-guide.md
$ git -C /path/to/ortho-config show \
    "v0.10.0^{commit}:docs/v0-10-0-migration-guide.md" | sha256sum
b5331aa6cfcc07b3dcd188c620db98d0a1a1f13b0c8ef96d88913d701bca25bf  -
$ sha256sum docs/ortho-config-v0-10-0-migration-guide.md
b5331aa6cfcc07b3dcd188c620db98d0a1a1f13b0c8ef96d88913d701bca25bf  \
    docs/ortho-config-v0-10-0-migration-guide.md
```

Each pair of hashes matching is the whole check, and both pairs are needed: the
migration guide is a copy of a different upstream file, so re-syncing one says
nothing about the other. If a pair differs, either that local copy drifted or
the tag was moved; `git rev-parse` settles which.

## Version boundary

Both guides describe `ortho_config` v0.10.0 and nothing later. Upstream text
that says a feature is "planned" or "not yet available" describes work that is
absent from v0.10.0, and so is absent here too. In particular, v0.10.0 does not
build with `default-features = false`
([leynos/ortho-config#627](https://github.com/leynos/ortho-config/issues/627)),
which is why the dependency in `Cargo.toml` carries default features.

## Where these guides are used

The design that consumes them is recorded in
[ADR 0014](adrs/0014-configuration-layering.md), and the delivery plan is
[the `ortho_config` adoption ExecPlan](execplans/issue-666-adopt-ortho-config-v0-10-0-for-the-mdtablefix-cli.md).
Neither the library nor the binary links against these documents; they are
reference material.
