# Scenario: student course selection

Pressure pass of keel through a classic enroll / drop / schedule flow.
Gates: L1 `tests/course.rs`, L2 `runseal :course` (also from `:guard`).

## Model (keel-api)

| Unit | Fields | Bonds |
|------|--------|-------|
| `Course` | `code`, `title` | — |
| `Student` | `no`, `name` | `courses` → Course (n2m) |

Enrollment is pure n2m: live ties only (no grade / term on the edge).

## Flow exercised

1. Create courses and students (POST).
2. Enroll (POST `/{student}/{id}/courses` with `right`).
3. Load schedule: `from Student where no = "…" link courses`, then
   `from Course where id in (…)` (H0 hydrate).
4. Drop one course (DELETE tie).
5. Patch student name.
6. Build a course roster by scanning `from Student link courses` and filtering
   `right` (no reverse edge, no association GET).
7. Soft-end a course; live course list shrinks.
8. GET on bond path stays 404.

## What worked

- Row write path: put / set / end / get / list.
- Edge write path: tie / cut over HTTP.
- Query pack + `link` + `id in` closes H0 client assembly.
- Root-only order/page is enough for student lists and course catalogs.

## Gaps found (not fixed in this pass)

| Gap | Impact | Direction |
|-----|--------|-----------|
| No edge business fields | No grade / term on enroll | **Settled: bond attrs** (`docs/bond.md`) |
| No reverse bond auto | Roster needs scan or P1 filter | R0 for now; not auto reverse |
| No edge `where` predicate | Cannot filter students by course | Open: P1/P2 in `docs/bond.md` |
| Ended course still in ties | bond bag vs hydrate length skew | Open: K* in `docs/bond.md` |
| Re-enroll after cut | UNIQUE(left,right) vs soft-cut | Open: U* in `docs/bond.md` |

### Cascading note

After `end(Course)`, a student `link courses` may still return a tie whose
`right` is no longer live. Clients that hydrate with `id in` get a shorter
course bag than the bond bag. Documented as open; cascade is not product law
yet.

## Commands

```sh
runseal :course   # full scenario L2
runseal :smoke    # thinner cold-start L2
runseal :guard    # includes both
```
