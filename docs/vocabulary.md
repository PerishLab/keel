# Vocabulary

Single-word naming is only livable against a maintained vocabulary. The vocabulary
is the set of atoms the codebase agrees to mean something, and it never freezes.

## Deltas, not just diffs

An agent that changes code also maintains the vocabulary. A new atom, a retired
atom, or a shifted meaning is a vocabulary delta, recorded alongside the code diff
so the shared meaning stays true.

## Current deltas

- `keel` — the library crate; the load-bearing data model description surface.
