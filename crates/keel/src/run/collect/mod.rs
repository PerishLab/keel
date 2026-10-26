use crate::adapt::Error;
use crate::estate::Gone;
use crate::model::manifest::Manifest;
use crate::plan::Plan;
use crate::wire::{Val, Wire};

mod tables;

pub(crate) use tables::Table;

pub(crate) struct Work<'a> {
    pub(crate) table: &'a Table,
    pub(crate) cutoff: i64,
    pub(crate) limit: usize,
    pub(crate) digest: &'a str,
}

pub(crate) fn tables(plan: &Plan) -> Result<Vec<Table>, Error> {
    tables::build(plan)
}

pub(crate) fn digest(plan: &Plan) -> String {
    Manifest::lift(plan).digest()
}

pub(crate) async fn run<W: Wire>(work: Work<'_>, wire: &mut W) -> Result<Vec<Gone>, Error> {
    let lock = match wire.grain() {
        crate::ddl::Grain::Lite => "",
        crate::ddl::Grain::Pg => " FOR UPDATE OF c SKIP LOCKED",
    };
    let text = format!(
        "SELECT c.id FROM {} AS c WHERE {} ORDER BY c.id LIMIT {}{lock}",
        work.table.place, work.table.test, work.limit
    );
    let rows = wire.rows(&text, &[Val::Int(work.cutoff)]).await?;
    if rows.is_empty() {
        return Ok(Vec::new());
    }
    let current = wire.rows(
        "SELECT g.digest FROM \"@generation\" AS g JOIN \"@estate\" AS e ON e.active = g.id WHERE e.id = 1 AND g.state = 'active'",
        &[],
    ).await?;
    if current.len() != 1 || current[0][0].text() != work.digest {
        return Err(Error::Adapt("collection model changed".into()));
    }
    let mut args = vec![Val::Int(work.cutoff)];
    args.extend(rows.into_iter().map(|row| row[0].clone()));
    let marks = (2..=args.len())
        .map(|i| format!("?{i}"))
        .collect::<Vec<_>>()
        .join(", ");
    let text = format!(
        "DELETE FROM {} AS c WHERE c.id IN ({marks}) AND {} RETURNING id",
        work.table.place, work.table.test
    );
    Ok(wire
        .rows(&text, &args)
        .await?
        .into_iter()
        .map(|row| Gone::new(work.table.path.clone(), row[0].int()))
        .collect())
}
