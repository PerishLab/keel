# keel

Personal-style data model description library (Rust).

Canonical source: [PerishLab/keel](https://git.perish.top/PerishLab/keel) on
`git.perish.top`.

## Status

Cold-start scaffold only. The model surface is not designed yet.

## Shape

- `crates/keel` — library crate
- `negentropy.toml` / `vocabulary.toml` — constitution
- `runseal.toml` / `.runseal/` — operator plane (`:init`, `:guard`, `:land`)
- `.forgejo/workflows/guard.yml` — CI gate on Forgejo Actions

## Operating

```sh
runseal :init
runseal :guard
```

Land a topic branch with `runseal :land` (push → PR → await `guard.yml` →
squash-merge).
