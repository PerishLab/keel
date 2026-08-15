# Migrating to Keel v0.10.4

This release changes authority behaviour. Read it before upgrading, because
two of the changes are visible to callers and one of them is visible to
clients.

No store changes. The format is untouched, nothing evolves on first open, and
no data is rewritten. Grant rows minted by earlier versions stay exactly where
they are: they are redundant now rather than wrong, they keep working under
union, and this release ships no sweep that removes them. An estate that wants
them gone can end them as ordinary rows once it has decided that is what it
wants.

## Coverage you were given by creating a row may no longer be there

Before, creating a row always minted the creator full coverage of it. Now that
grant is written only when nothing the creator already holds covers the row.

You are unaffected if the coverage you rely on is the one that let you create
the row in the first place — a subtree grant, an `all` scope, or a predicate
that still matches. The rows you create stay reachable through it.

You are affected if you relied on the created row outliving that coverage.
Two shapes to check:

- An operator creates rows under a grant you later revoke or narrow. Before,
  they kept every row they had made. Now they lose them with the grant. If
  that access was intended to survive, issue an explicit grant for it —
  which is what it always was, only written down.
- An operator creates a row under a **predicate** grant covering `put` alone.
  Nothing changes there: a predicate covers one verb, so it does not cover the
  new row for `see`, `set` or `end`, and the mint still happens. If you have
  been leaning on that mint, it is still under you — but it is worth deciding
  whether you meant to grant those verbs.

If you watch the pulse stream for `@grant` puts as a proxy for row creation,
that signal is gone for covered creations. Watch the unit you actually care
about instead.

## Handing a row to someone else now works, and ends your reach

A `set` that moves a row out of your own subtree used to be refused unless you
held row coverage independent of the chain — in practice, the coverage the
mint had handed you. It now succeeds, and afterwards the row is not yours.

Nothing was loosened for taking: a row you cannot reach still fails the check
that runs before the write. What changed is that giving is no longer forbidden
by the check that runs after it.

If your product deliberately forbade transfer by relying on that refusal, it
is no longer forbidden by the engine. Express it as a grant instead.

## `PATCH` answers 204 when the writer can no longer see the row

The route wrote the row, read it back as the same operator, and answered `404`
when that read found nothing — which reported failure for a write that had
succeeded. It now answers `204 No Content`.

A client that treats every non-2xx as failure was already wrong here and is now
correct by accident. A client that treats `404` as "the write did not happen"
is now correct: `404` from `PATCH` means nothing was written.

## `allows` was never implemented

`DESIGN.md` and `ARCHITECTURE.md` described a non-mutating `allows` probe. No
`Face` has ever exposed it. The sentences are removed. No code changes, because
there was no code.

## What this is worth

Writes stop growing with the estate. On the readings shipped with this release,
a write that mints climbed from 0.92ms to 6.23ms as an estate accumulated 1800
grants; after this release the same write measures 0.20ms at any size, and the
authority ledger no longer tracks data volume at all.
