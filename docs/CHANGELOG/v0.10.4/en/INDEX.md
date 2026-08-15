# Keel v0.10.4

## Creating a row no longer mints coverage a grant already carries

Every write by an operator used to write a second row: a `@grant` giving the
creator full coverage of what they had just made. That grant is now written
only when nothing the creator already holds covers the new row.

It was never free, and it was never only the row. Writing `@grant` bumps that
unit's generation, so the next authority check re-read every grant row — and
the table it re-read had just grown by one. Creating rows was quadratic, and
the ledger of authority grew one row per row of data, permanently.

Measured on the readings that ship with this release
(`cargo test -p keel --test cost -- --ignored --nocapture --test-threads=1`),
three hundred writes per round against in-memory SQLite:

    round   put before   put after   grants before   grants after
    1       0.92ms       0.20ms      303             3
    3       2.67ms       0.20ms      903             3
    6       6.23ms       0.22ms      1803            3

A write that does not mint — `set` — measured 0.24ms flat throughout, before
and after. That pair is the whole finding: authority checking was never the
expensive part, minting was, and minting was what made checking expensive.

The rule that decides is the mirror of one Keel already had. A grant an
operator could not have issued from live coverage is refused; a grant that
merely repeats coverage the operator already holds carries nothing, so it is
not written. Anonymous identity birth keeps its unconditional mint, because a
newborn identity holds nothing yet.

## A write is authorized by the state it started in

Row and subtree coverage now answers the state a `set` began with. A predicate
is still required before and after, because a predicate describes the row's
content and a write must not falsify it.

This followed from the first change rather than being chosen beside it. A
holder who covers a row through its root chain could hand it to someone else
only because the mint had given them coverage independent of that chain: the
after-check re-anchors the chain, and after the move the row hangs under the
receiver. Handing a thing over is exactly the write that ends your own reach,
so requiring reach afterwards forbade it permanently.

Taking is unaffected. A row you cannot reach fails the check that runs first,
which is the one that was doing the work all along.

## A successful write that leaves your sight answers 204

`PATCH` read the row back as the operator who had just written it, and reported
`404` when that read found nothing — after the write had already happened. The
route now answers `204 No Content` in that case. `404` remains what it always
meant: nothing was written.

## `allows` left the documents

`DESIGN.md` and `ARCHITECTURE.md` both described a non-mutating probe that no
`Face` implements and no test calls. The projections are sealed on the claim
that a person read both sides, so a promise the code does not keep is removed
rather than carried. If the probe is wanted, it can arrive with its own tests.

## Under the hood

`check`, `shift`, `spans` and `broad` were one receiver wearing four names and
are now `Court`; what a deed is asked to decide is one `Case`.
