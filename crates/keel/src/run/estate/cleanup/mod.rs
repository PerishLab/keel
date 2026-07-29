use crate::adapt::Error;
use crate::config::Cleanup;
use crate::model::manifest::Manifest;
use crate::wire::{Val, Wire};
use std::collections::BTreeMap;

mod derivative;
mod hook;

pub use hook::{Gone, Hook, Purge};

pub(super) async fn run<W: Wire>(
    policy: &Cleanup,
    active: &Manifest,
    wire: &mut W,
) -> Result<(), Error> {
    wire.script("BEGIN").await?;
    let out = sweep(policy, active, wire).await;
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

async fn sweep<W: Wire>(policy: &Cleanup, active: &Manifest, wire: &mut W) -> Result<(), Error> {
    let rows = wire
        .rows(
            "SELECT id, retired FROM \"@generation\" WHERE state = ?1 ORDER BY id",
            &[Val::Text("cleanup".into())],
        )
        .await?;
    let now = crate::life::tick();
    let mut due = Vec::new();
    for row in rows {
        if row.len() != 2 {
            return Err(unknown("cleanup record shape"));
        }
        let retired = row[1].opt().ok_or_else(|| unknown("cleanup retirement"))?;
        if policy.retain.elapsed(retired, now) {
            due.push(row[0].int());
        }
    }
    if due.is_empty() {
        return Ok(());
    }
    let frame = crate::plan::Plan::meta();
    let mut due = {
        let mut held = Vec::new();
        for generation in due {
            let lines = crate::life::Work::new(wire, &frame)
                .recall(generation)
                .await?;
            let manifest = crate::model::manifest::rows::Sheet(&lines)
                .gather()
                .map_err(|note| unknown(&format!("cleanup schema {note}")))?;
            held.push((generation, manifest));
        }
        held
    };
    due.sort_by_key(|(at, _)| *at);
    for (generation, manifest) in due {
        let gone = derivative::gone(generation, &manifest, active, wire).await?;
        save(generation, &gone, now, wire).await?;
        for table in super::tables::of(&manifest) {
            let table = crate::ddl::stage(generation, &table);
            wire.script(&format!("DROP TABLE {}", crate::ddl::col(&table)))
                .await?;
        }
        let removed = wire
            .run(
                "DELETE FROM \"@generation\" WHERE id = ?1 AND state = ?2",
                &[Val::Int(generation), Val::Text("cleanup".into())],
            )
            .await?;
        if removed != 1 {
            return Err(unknown("cleanup generation changed"));
        }
        crate::life::Work::new(wire, &frame)
            .purge(generation)
            .await?;
    }
    let shape = super::catalog::Catalog(wire).shape().await?;
    let changed = wire
        .run(
            "UPDATE \"@estate\" SET shape = ?1 WHERE id = 1",
            &[Val::Text(shape)],
        )
        .await?;
    if changed != 1 {
        return Err(unknown("cleanup estate root"));
    }
    Ok(())
}

async fn save<W: Wire>(
    generation: i64,
    gone: &[Gone],
    created: i64,
    wire: &mut W,
) -> Result<(), Error> {
    for atom in gone {
        wire.run(
            "INSERT INTO \"@derivative\" (generation, path, key, created) VALUES (?1, ?2, ?3, ?4)",
            &[
                Val::Int(generation),
                Val::Text(atom.path().into()),
                Val::Int(atom.key()),
                Val::Int(created),
            ],
        )
        .await?;
    }
    Ok(())
}

pub(crate) async fn deliver<H: Hook, W: Wire>(
    hook: Option<&mut H>,
    wire: &mut W,
) -> Result<(), Error> {
    let Some(hook) = hook else {
        return Ok(());
    };
    let rows = wire
        .rows(
            "SELECT generation, path, key, created FROM \"@derivative\" ORDER BY generation, path, key",
            &[],
        )
        .await?;
    let mut pending: BTreeMap<i64, Vec<Gone>> = BTreeMap::new();
    for row in rows {
        if row.len() != 4 || row[3].int() <= 0 {
            return Err(unknown("derivative record shape"));
        }
        pending
            .entry(row[0].int())
            .or_default()
            .push(Gone::new(row[1].text(), row[2].int()));
    }
    for (generation, gone) in pending {
        let event = Purge::new(generation, gone);
        if let Err(note) = hook.purge(event.clone()).await {
            eprintln!("keel: derivative hook: {note}");
            continue;
        }
        let removed = wire
            .run(
                "DELETE FROM \"@derivative\" WHERE generation = ?1",
                &[Val::Int(generation)],
            )
            .await?;
        if removed != event.gone().len() as u64 {
            return Err(unknown("derivative acknowledgement"));
        }
    }
    Ok(())
}

fn unknown(note: &str) -> Error {
    Error::Estate(super::Fault::Unknown(note.into()))
}
