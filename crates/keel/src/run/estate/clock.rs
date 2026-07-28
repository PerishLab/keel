use crate::adapt::Error;
use crate::wire::{Val, Wire};

pub(crate) fn unit(key: &str) -> String {
    format!("unit:{key}")
}

pub(crate) fn bond(unit: &str, edge: &str) -> String {
    format!("bond:{unit}.{edge}")
}

pub(crate) fn serial(unit: &str, slot: &str, scope: &Val) -> Result<String, Error> {
    let scope = match scope {
        Val::Null => "null".into(),
        Val::Int(value) => format!("int:{value}"),
        Val::Text(value) => format!("text:{}:{value}", value.len()),
    };
    Ok(format!("serial:{unit}:{slot}:{scope}"))
}

pub(crate) async fn next<W: Wire>(wire: &mut W, name: &str) -> Result<i64, Error> {
    let rows = wire
        .rows(
            "INSERT INTO \"@clock\" (name, value) VALUES (?1, 1) ON CONFLICT (name) DO UPDATE SET value = \"@clock\".value + 1 RETURNING value",
            &[Val::Text(name.into())],
        )
        .await?;
    rows.first()
        .and_then(|row| row.first())
        .map(Val::int)
        .ok_or_else(|| Error::Estate(super::Fault::Unknown(format!("clock {name}"))))
}

pub(super) async fn hold<W: Wire>(wire: &mut W, name: String, value: i64) -> Result<(), Error> {
    if value <= 0 {
        return Ok(());
    }
    let changed = wire
        .run(
            "INSERT INTO \"@clock\" (name, value) VALUES (?1, ?2)",
            &[Val::Text(name), Val::Int(value)],
        )
        .await?;
    if changed != 1 {
        return Err(Error::Estate(super::Fault::Unknown("clock restore".into())));
    }
    Ok(())
}
