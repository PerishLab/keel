use super::Fault;
use super::catalog::{Catalog, frame};
use crate::adapt::Error;
use crate::ddl::Grain;
use crate::model::manifest::digest;
use crate::plan::Plan;
use crate::wire::{Val, Wire};
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(1);

type Row = Vec<Val>;

struct Projection {
    script: Vec<String>,
}

pub(super) async fn shape<W: Wire>(plan: &Plan, wire: &mut W) -> Result<String, Error> {
    Ok(frame(&rows(plan, 1, wire).await?))
}

async fn rows<W: Wire>(plan: &Plan, generation: i64, wire: &mut W) -> Result<Vec<Row>, Error> {
    let script = crate::ddl::settled(plan, wire.grain(), generation);
    Projection { script }.rows(wire).await
}

impl Projection {
    async fn rows<W: Wire>(&self, wire: &mut W) -> Result<Vec<Row>, Error> {
        match wire.grain() {
            Grain::Lite => self.lite().await,
            Grain::Pg => self.pg(wire).await,
        }
    }

    async fn lite(&self) -> Result<Vec<Row>, Error> {
        let mut wire = crate::adapt::db::Sqlite::memory().await?;
        self.build(&mut wire).await?;
        Catalog(&mut wire).rows().await
    }

    async fn pg<W: Wire>(&self, wire: &mut W) -> Result<Vec<Row>, Error> {
        wire.script("BEGIN").await?;
        let out = self.shadow(wire).await;
        let undone = wire.script("ROLLBACK").await;
        match (out, undone) {
            (Ok(shape), Ok(())) => Ok(shape),
            (Err(err), _) => Err(err),
            (Ok(_), Err(err)) => Err(err),
        }
    }

    async fn shadow<W: Wire>(&self, wire: &mut W) -> Result<Vec<Row>, Error> {
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let name = format!(
            "keel_adopt_{}_{}_{}",
            std::process::id(),
            crate::life::tick(),
            id
        );
        wire.script(&format!("CREATE SCHEMA {}", crate::ddl::col(&name)))
            .await?;
        wire.script(&format!(
            "SET LOCAL search_path TO {}",
            crate::ddl::col(&name)
        ))
        .await?;
        self.build(wire).await?;
        Catalog(wire).rows().await
    }

    async fn build<W: Wire>(&self, wire: &mut W) -> Result<(), Error> {
        for stmt in &self.script {
            wire.script(stmt).await?;
        }
        Ok(())
    }
}

pub(super) async fn gain<W: Wire>(plan: &Plan, wire: &mut W) -> Result<(), Error> {
    let generation = super::verify(wire).await?.generation;
    let expected = rows(plan, generation, wire).await?;
    let found = Catalog(&mut *wire).rows().await?;
    if gap(wire.grain(), &expected, &found)?.is_empty() {
        return Ok(());
    }
    wire.script("BEGIN").await?;
    let out = build(generation, &expected, wire).await;
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

async fn build<W: Wire>(generation: i64, expected: &[Row], wire: &mut W) -> Result<(), Error> {
    wire.run("UPDATE \"@estate\" SET shape = shape WHERE id = 1", &[])
        .await?;
    if super::verify(wire).await?.generation != generation {
        return Err(Error::Estate(Fault::Unknown(
            "estate generation moved".into(),
        )));
    }
    let grain = wire.grain();
    let found = Catalog(&mut *wire).rows().await?;
    let gap = gap(grain, expected, &found)?;
    for row in &gap {
        let (name, text) = index(grain, row);
        wire.script(&text).await.map_err(|err| {
            Error::Estate(Fault::Denied {
                path: name,
                note: err.to_string(),
            })
        })?;
    }
    let after = Catalog(&mut *wire).rows().await?;
    let mut want: Vec<String> = found.iter().chain(&gap).map(key).collect();
    let mut got: Vec<String> = after.iter().map(key).collect();
    want.sort();
    got.sort();
    if want != got {
        return Err(Error::Estate(Fault::Drift {
            expected: digest(&want.concat()),
            found: digest(&got.concat()),
        }));
    }
    wire.run(
        "UPDATE \"@estate\" SET shape = ?1 WHERE id = 1",
        &[Val::Text(frame(&after))],
    )
    .await?;
    Ok(())
}

fn gap(grain: Grain, expected: &[Row], found: &[Row]) -> Result<Vec<Row>, Error> {
    let tables: BTreeSet<String> = expected.iter().map(|row| owner(grain, row)).collect();
    let held: Vec<&Row> = found
        .iter()
        .filter(|row| tables.contains(&owner(grain, row)))
        .collect();
    let seated: BTreeSet<String> = held
        .iter()
        .filter(|row| row[0].text() == "table")
        .map(|row| row[1].text())
        .collect();
    let want: BTreeSet<String> = expected.iter().filter(|row| kept(row)).map(key).collect();
    let have: BTreeSet<String> = held
        .iter()
        .filter(|row| kept(row))
        .map(|row| key(row))
        .collect();
    let whole = expected
        .iter()
        .filter(|row| row[0].text() == "table")
        .all(|row| seated.contains(&row[1].text()));
    if whole && have.is_subset(&want) {
        return Ok(expected
            .iter()
            .filter(|row| kept(row) && !have.contains(&key(row)))
            .cloned()
            .collect());
    }
    let held: Vec<Row> = held.into_iter().cloned().collect();
    Err(Error::Estate(Fault::Drift {
        expected: digest(&frame(expected)),
        found: digest(&frame(&held)),
    }))
}

fn kept(row: &Row) -> bool {
    row[0].text() == "index"
}

fn owner(grain: Grain, row: &Row) -> String {
    match grain {
        Grain::Lite => row[2].text(),
        Grain::Pg if row[0].text() == "table" => row[1].text(),
        Grain::Pg => {
            let name = row[1].text();
            name.split_once('.')
                .map_or(name.clone(), |(table, _)| table.into())
        }
    }
}

fn index(grain: Grain, row: &Row) -> (String, String) {
    match grain {
        Grain::Lite => (row[1].text(), row[3].text()),
        Grain::Pg => {
            let name = row[1].text();
            let name = name
                .split_once('.')
                .map_or(name.clone(), |(_, index)| index.into());
            (name, row[2].text())
        }
    }
}

fn key(row: &Row) -> String {
    frame(std::slice::from_ref(row))
}
