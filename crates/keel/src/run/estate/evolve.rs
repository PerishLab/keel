use super::Change;
use crate::adapt::Error;
use crate::model::manifest::Manifest;
use crate::plan::Plan;
use crate::wire::{Val, Wire};

pub(super) struct Work<'a> {
    pub(super) plan: &'a Plan,
    pub(super) active: &'a Manifest,
    pub(super) requested: &'a Manifest,
    pub(super) generation: i64,
    pub(super) change: &'a Change,
}

struct Swap<'a> {
    active: &'a Manifest,
    requested: &'a Manifest,
    old: i64,
    new: i64,
}

pub(super) async fn run<W: Wire>(work: Work<'_>, wire: &mut W) -> Result<(), Error> {
    wire.script("BEGIN").await?;
    let out = migrate(&work, wire).await;
    match out {
        Ok(()) => {
            wire.script("COMMIT").await?;
            Ok(())
        }
        Err(err) => {
            let _ = wire.script("ROLLBACK").await;
            Err(err)
        }
    }
}

async fn migrate<W: Wire>(work: &Work<'_>, wire: &mut W) -> Result<(), Error> {
    let Work {
        plan,
        active,
        requested,
        generation,
        change,
    } = work;
    super::guard::run(
        super::guard::Scope {
            change,
            active,
            requested,
            plan,
        },
        wire,
    )
    .await?;
    let next = super::next(wire, "generation").await?;
    let text = requested.write();
    wire.run(
        "INSERT INTO \"@generation\" (id, state, digest, manifest, created, retired) VALUES (?1, ?2, ?3, ?4, ?5, NULL)",
        &[
            Val::Int(next),
            Val::Text("candidate".into()),
            Val::Text(requested.digest()),
            Val::Text(text),
            Val::Int(crate::life::tick()),
        ],
    )
    .await?;
    for stmt in crate::ddl::candidate(plan, wire.grain(), next) {
        wire.script(&stmt).await?;
    }
    super::copy::run(active, requested, next, wire).await?;
    activate(
        Swap {
            active,
            requested,
            old: *generation,
            new: next,
        },
        wire,
    )
    .await?;
    contract(active, requested, wire).await?;
    wire.run(
        "UPDATE \"@generation\" SET state = ?1, retired = ?2 WHERE id = ?3",
        &[
            Val::Text("cleanup".into()),
            Val::Int(crate::life::tick()),
            Val::Int(*generation),
        ],
    )
    .await?;
    wire.run(
        "UPDATE \"@generation\" SET state = ?1 WHERE id = ?2",
        &[Val::Text("active".into()), Val::Int(next)],
    )
    .await?;
    let shape = super::catalog::Catalog(wire).shape().await?;
    wire.run(
        "UPDATE \"@estate\" SET active = ?1, shape = ?2 WHERE id = 1",
        &[Val::Int(next), Val::Text(shape)],
    )
    .await?;
    let mut rows = crate::life::Work::new(wire, plan);
    rows.erase().await?;
    rows.etch(&crate::model::manifest::rows::spill(requested))
        .await?;
    Ok(())
}

async fn activate<W: Wire>(swap: Swap<'_>, wire: &mut W) -> Result<(), Error> {
    for table in super::tables::of(swap.active) {
        rename(wire, &table, &crate::ddl::stage(swap.old, &table)).await?;
    }
    for table in super::tables::of(swap.requested) {
        rename(wire, &crate::ddl::stage(swap.new, &table), &table).await?;
    }
    Ok(())
}

async fn rename<W: Wire>(wire: &mut W, from: &str, to: &str) -> Result<(), Error> {
    let text = format!(
        "ALTER TABLE {} RENAME TO {}",
        crate::ddl::col(from),
        crate::ddl::col(to)
    );
    wire.script(&text).await
}

async fn contract<W: Wire>(
    active: &Manifest,
    requested: &Manifest,
    wire: &mut W,
) -> Result<(), Error> {
    let old = paths(active);
    let new = paths(requested);
    for path in old.iter().filter(|path| !new.contains(*path)) {
        wire.run(
            "DELETE FROM \"@pulse\" WHERE unit = ?1",
            &[Val::Text(path.clone())],
        )
        .await?;
    }
    Ok(())
}

fn paths(manifest: &Manifest) -> Vec<String> {
    let mut out = Vec::new();
    for unit in manifest.units() {
        out.push(unit.key.clone());
        out.extend(
            unit.bonds
                .iter()
                .map(|edge| format!("{}.{}", unit.key, edge.name)),
        );
    }
    out
}
