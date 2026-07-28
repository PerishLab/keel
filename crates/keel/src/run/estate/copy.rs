use crate::adapt::Error;
use crate::ddl::Grain;
use crate::model::manifest::{Atom, Bond, Edge, Field, Manifest, Unit};
use crate::wire::{Val, Wire};

struct Pair<'a> {
    active: &'a Edge,
    requested: &'a Edge,
}

struct Sheet {
    target: String,
    source: String,
    cols: Vec<String>,
    vals: Vec<String>,
}

pub(super) async fn run<W: Wire>(
    active: &Manifest,
    requested: &Manifest,
    generation: i64,
    wire: &mut W,
) -> Result<(), Error> {
    for target in requested.units() {
        let Some(prior) = active.units().iter().find(|old| old.key == target.key) else {
            continue;
        };
        unit(prior, target, generation, wire).await?;
        for edge in target
            .bonds
            .iter()
            .filter(|edge| edge.kind == Bond::Many2many)
        {
            let Some(old) = prior.bond(&edge.name) else {
                continue;
            };
            bond(
                prior,
                Pair {
                    active: old,
                    requested: edge,
                },
                generation,
                wire,
            )
            .await?;
        }
    }
    Ok(())
}

async fn unit<W: Wire>(
    active: &Unit,
    requested: &Unit,
    generation: i64,
    wire: &mut W,
) -> Result<(), Error> {
    let mut cols = vec![crate::ddl::KEY.into()];
    let mut vals = vec![crate::ddl::KEY.into()];
    for field in &requested.fields {
        cols.push(crate::ddl::col(&field.name));
        vals.push(scalar(active, field, wire.grain()));
    }
    for edge in requested.bonds.iter().filter(|edge| point(edge.kind)) {
        cols.push(crate::ddl::col(&crate::ddl::side(&edge.name)));
        let value = active
            .bond(&edge.name)
            .filter(|old| point(old.kind))
            .map(|_| crate::ddl::col(&crate::ddl::side(&edge.name)))
            .unwrap_or_else(|| "NULL".into());
        vals.push(value);
    }
    reign(&mut cols, &mut vals);
    let sheet = Sheet {
        target: crate::ddl::stage(generation, &requested.table()),
        source: active.table(),
        cols,
        vals,
    };
    transfer(wire, &sheet).await
}

async fn bond<W: Wire>(
    owner: &Unit,
    pair: Pair<'_>,
    generation: i64,
    wire: &mut W,
) -> Result<(), Error> {
    let active = pair.active;
    let requested = pair.requested;
    let src = crate::ddl::side(&owner.name);
    let dst = crate::ddl::mate(&owner.name, &requested.name, &requested.target);
    let mut cols = vec![
        crate::ddl::KEY.into(),
        crate::ddl::col(&src),
        crate::ddl::col(&dst),
    ];
    let mut vals = cols.clone();
    for slot in &requested.fields {
        cols.push(crate::ddl::col(&slot.name));
        vals.push(field(active, slot, wire.grain()));
    }
    reign(&mut cols, &mut vals);
    let table = crate::ddl::joiner(&owner.table(), &requested.name);
    let sheet = Sheet {
        target: crate::ddl::stage(generation, &table),
        source: table,
        cols,
        vals,
    };
    transfer(wire, &sheet).await
}

fn scalar(active: &Unit, requested: &Field, grain: Grain) -> String {
    let value = active
        .fields
        .iter()
        .find(|field| field.name == requested.name)
        .map(|field| cast(field, requested, grain))
        .unwrap_or_else(|| "NULL".into());
    fill(requested, value)
}

fn field(active: &Edge, requested: &Field, grain: Grain) -> String {
    active
        .fields
        .iter()
        .find(|field| field.name == requested.name)
        .map(|field| cast(field, requested, grain))
        .unwrap_or_else(|| "NULL".into())
}

fn cast(active: &Field, requested: &Field, grain: Grain) -> String {
    let col = crate::ddl::col(&active.name);
    match (active.kind, requested.kind) {
        (Atom::Text, Atom::Int) => {
            let kind = if grain == Grain::Lite {
                "INTEGER"
            } else {
                "BIGINT"
            };
            format!("CAST({col} AS {kind})")
        }
        (Atom::Text, Atom::Bool) => {
            format!("CASE WHEN {col} IS NULL THEN NULL WHEN {col} = 'true' THEN 1 ELSE 0 END")
        }
        (Atom::Int, Atom::Text) => format!("CAST({col} AS TEXT)"),
        (Atom::Bool, Atom::Text) => {
            format!("CASE WHEN {col} IS NULL THEN NULL WHEN {col} = 1 THEN 'true' ELSE 'false' END")
        }
        _ => col,
    }
}

fn fill(field: &Field, value: String) -> String {
    match field.rule.default.as_deref() {
        Some(default) => format!("COALESCE({value}, {})", literal(field.kind, default)),
        None => value,
    }
}

fn literal(kind: Atom, value: &str) -> String {
    match kind {
        Atom::Text | Atom::Link => format!("'{}'", value.replace('\'', "''")),
        Atom::Int => value.to_string(),
        Atom::Bool => match value {
            "true" => "1".into(),
            "false" => "0".into(),
            _ => unreachable!("normalized bool"),
        },
    }
}

fn reign(cols: &mut Vec<String>, vals: &mut Vec<String>) {
    for name in [
        crate::ddl::EXPIRES,
        crate::ddl::CREATED,
        crate::ddl::UPDATED,
    ] {
        cols.push(name.into());
        vals.push(name.into());
    }
}

async fn transfer<W: Wire>(wire: &mut W, sheet: &Sheet) -> Result<(), Error> {
    let text = format!(
        "INSERT INTO {} ({}) SELECT {} FROM {} WHERE {} IS NULL OR {} > ?1",
        crate::ddl::col(&sheet.target),
        sheet.cols.join(", "),
        sheet.vals.join(", "),
        crate::ddl::col(&sheet.source),
        crate::ddl::EXPIRES,
        crate::ddl::EXPIRES
    );
    wire.run(&text, &[Val::Int(crate::life::tick())]).await?;
    Ok(())
}

fn point(kind: Bond) -> bool {
    matches!(kind, Bond::Many2one | Bond::One2one)
}
