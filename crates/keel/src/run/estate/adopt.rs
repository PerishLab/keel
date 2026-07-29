use crate::adapt::Error;
use crate::bond;
use crate::model::manifest::Manifest;
use crate::plan::{Plan, Unit};
use crate::wire::{Val, Wire};
use std::collections::BTreeMap;

pub(crate) struct Policy<'a> {
    pub(crate) cleanup: &'a crate::config::Cleanup,
    pub(crate) adopt: bool,
}

pub(super) async fn run<W: Wire>(
    plan: &Plan,
    manifest: &Manifest,
    wire: &mut W,
) -> Result<(), Error> {
    let expected = super::projection::shape(plan, wire).await?;
    wire.script("BEGIN").await?;
    let out = stamp(plan, manifest, &expected, wire).await;
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

async fn stamp<W: Wire>(
    plan: &Plan,
    manifest: &Manifest,
    expected: &str,
    wire: &mut W,
) -> Result<(), Error> {
    let found = super::catalog::Catalog(wire).shape().await?;
    if found != expected {
        return Err(Error::Estate(super::Fault::Drift {
            expected: crate::model::manifest::digest(expected),
            found: crate::model::manifest::digest(&found),
        }));
    }
    for stmt in super::catalog::script(wire.grain()) {
        wire.script(&stmt).await?;
    }
    clocks(plan, wire).await?;
    let generation = 1;
    super::clock::hold(wire, "generation".into(), generation).await?;
    wire.run(
        "INSERT INTO \"@generation\" (id, state, digest, created, retired) VALUES (?1, ?2, ?3, ?4, NULL)",
        &[
            Val::Int(generation),
            Val::Text("active".into()),
            Val::Text(manifest.digest()),
            Val::Int(crate::life::tick()),
        ],
    )
    .await?;
    let mut rows = crate::life::Work::new(wire, plan);
    rows.erase().await?;
    rows.etch(&crate::model::manifest::rows::spill(manifest), generation)
        .await?;
    let shape = super::catalog::Catalog(wire).shape().await?;
    wire.run(
        "INSERT INTO \"@estate\" (id, format, active, shape) VALUES (?1, ?2, ?3, ?4)",
        &[
            Val::Int(1),
            Val::Int(super::catalog::FORMAT),
            Val::Int(generation),
            Val::Text(shape),
        ],
    )
    .await?;
    Ok(())
}

async fn clocks<W: Wire>(plan: &Plan, wire: &mut W) -> Result<(), Error> {
    for unit in plan.units().values() {
        let high = maximum(&unit.table(), wire).await?;
        super::clock::hold(wire, super::clock::unit(&unit.key()), high).await?;
        bonds(unit, wire).await?;
        serials(unit, wire).await?;
    }
    let pulse = plan.find(crate::cap::PULSE)?;
    let high = maximum(&pulse.table(), wire).await?;
    super::clock::hold(wire, "pulse".into(), high).await
}

async fn bonds<W: Wire>(unit: &Unit, wire: &mut W) -> Result<(), Error> {
    for edge in unit
        .bonds()
        .iter()
        .filter(|edge| edge.kind() == bond::Kind::Many2many)
    {
        let table = crate::ddl::join(unit, edge.name());
        let high = maximum(&table, wire).await?;
        let name = super::clock::bond(&unit.key(), edge.name());
        super::clock::hold(wire, name, high).await?;
    }
    Ok(())
}

async fn serials<W: Wire>(unit: &Unit, wire: &mut W) -> Result<(), Error> {
    for slot in unit.fields().iter().filter(|slot| slot.serial().is_some()) {
        let scope = slot.serial().expect("serial scope");
        let text = format!(
            "SELECT {}, {} FROM {}",
            crate::ddl::col(&crate::ddl::side(scope)),
            crate::ddl::col(slot.name()),
            crate::ddl::col(&unit.table())
        );
        let rows = wire.rows(&text, &[]).await?;
        let mut highs = BTreeMap::new();
        for row in rows {
            if row.len() != 2 {
                return Err(unknown("serial record shape"));
            }
            let name = super::clock::serial(&unit.key(), slot.name(), &row[0])?;
            let high = highs.entry(name).or_insert(0);
            *high = (*high).max(row[1].int());
        }
        for (name, high) in highs {
            super::clock::hold(wire, name, high).await?;
        }
    }
    Ok(())
}

async fn maximum<W: Wire>(table: &str, wire: &mut W) -> Result<i64, Error> {
    let rows = wire
        .rows(
            &format!(
                "SELECT MAX({}) FROM {}",
                crate::ddl::KEY,
                crate::ddl::col(table)
            ),
            &[],
        )
        .await?;
    if rows.len() != 1 || rows[0].len() != 1 {
        return Err(unknown("clock source shape"));
    }
    Ok(rows[0][0].opt().unwrap_or(0))
}

fn unknown(note: &str) -> Error {
    Error::Estate(super::Fault::Unknown(note.into()))
}
