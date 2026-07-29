use super::util::*;
use super::*;
use crate::adapt::Error;
use crate::bond;
use crate::ddl;
use crate::plan::{Edge, Plan, Slot, Unit};
use crate::spec::Only;
use crate::wire::{Val, Wire};

impl<'a, W: Wire> Work<'a, W> {
    pub(crate) fn new(wire: &'a mut W, plan: &'a Plan) -> Self {
        Self { wire, plan }
    }

    pub(crate) async fn put(&mut self, name: &str, fields: &[(&str, &str)]) -> Result<i64, Error> {
        let unit = self.plan.find(name)?;
        if unit.name() == crate::cap::PULSE {
            return Err(Error::Adapt("pulse is engine owned".into()));
        }
        if crate::plan::meta::owned(unit.name()) {
            return Err(Error::Adapt("schema is engine owned".into()));
        }
        if unit.name() == crate::cap::GRANT {
            crate::cap::vet(self.plan, fields)?;
        }
        self.craft(unit, fields).await
    }

    pub(crate) async fn craft(
        &mut self,
        unit: &'a Unit,
        fields: &[(&str, &str)],
    ) -> Result<i64, Error> {
        unit.check(fields)?;
        let tick = now();
        let mut cols: Vec<String> = vec![ddl::KEY.into()];
        cols.extend(unit.fields().iter().map(|s| ddl::col(s.name())));
        for edge in unit.refs() {
            cols.push(ddl::col(&ddl::side(edge.name())));
        }
        cols.push(ddl::EXPIRES.into());
        cols.push(ddl::CREATED.into());
        cols.push(ddl::UPDATED.into());
        let marks = (1..=cols.len())
            .map(|i| format!("?{i}"))
            .collect::<Vec<_>>()
            .join(", ");
        let text = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            ddl::seat(unit),
            cols.join(", "),
            marks
        );
        let clock = crate::estate::clock::unit(&unit.key());
        let key = crate::estate::next(self.wire, &clock).await?;
        let mut vals: Vec<Val> = vec![Val::Int(key)];
        for slot in unit.fields() {
            if let Some(scope) = slot.serial() {
                vals.push(self.next(unit, slot, scope, fields).await?);
                continue;
            }
            vals.push(match seek(fields, slot.name()) {
                Some(hit) => slot.bind(hit)?,
                None if !slot.need() => Val::Null,
                None if slot.rule().fallback().is_some() => {
                    slot.bind(slot.rule().fallback().expect("fallback"))?
                }
                None => {
                    return Err(Error::Adapt(format!("missing field {}", slot.name())));
                }
            });
        }
        for edge in unit.refs() {
            let hit = pluck(fields, edge.name());
            vals.push(self.point(unit, edge, hit, None).await?);
        }
        vals.push(Val::Null);
        vals.push(Val::Int(tick));
        vals.push(Val::Int(tick));
        self.solid(unit, fields, None).await?;
        self.wire.run(&text, &vals).await?;
        Ok(key)
    }

    pub(super) async fn solid(
        &mut self,
        unit: &Unit,
        fields: &[(&str, &str)],
        myself: Option<(i64, &Row)>,
    ) -> Result<(), Error> {
        for slot in unit.fields() {
            if *slot.only() == Only::Free {
                continue;
            }
            self.taken(unit, slot, fields, myself).await?;
        }
        Ok(())
    }

    pub(super) async fn taken(
        &mut self,
        unit: &Unit,
        slot: &Slot,
        fields: &[(&str, &str)],
        myself: Option<(i64, &Row)>,
    ) -> Result<(), Error> {
        let Some(value) = worth(slot, fields, myself)? else {
            return Ok(());
        };
        let me = myself.map(|(key, _)| key).unwrap_or(0);
        let mut text = format!(
            "SELECT 1 FROM {} WHERE {} = ?1 AND {} != ?2 AND ({} IS NULL OR {} > ?3)",
            ddl::seat(unit),
            ddl::col(slot.name()),
            ddl::KEY,
            ddl::EXPIRES,
            ddl::EXPIRES
        );
        let mut vals = vec![value, Val::Int(me), Val::Int(now())];
        if let Only::Per(rels) = slot.only() {
            for rel in rels {
                let col = unit.column(rel)?;
                let at = vals.len() + 1;
                text.push_str(&format!(
                    " AND ({col} = ?{at} OR (?{at} IS NULL AND {col} IS NULL))"
                ));
                vals.push(anchor(unit, fields, myself, rel)?);
            }
        }
        text.push_str(" LIMIT 1");
        if !self.wire.rows(&text, &vals).await?.is_empty() {
            return Err(Error::Adapt(format!("field {} taken", slot.name())));
        }
        Ok(())
    }

    pub(super) async fn point(
        &mut self,
        unit: &Unit,
        edge: &Edge,
        value: &str,
        myself: Option<i64>,
    ) -> Result<Val, Error> {
        if value.is_empty() {
            if edge.need() {
                return Err(Error::Adapt(format!("missing ref {}", edge.name())));
            }
            return Ok(Val::Null);
        }
        let key = value
            .parse::<i64>()
            .map_err(|_| Error::Adapt(format!("ref {} needs id", edge.name())))?;
        let mate = self.plan.find(edge.target())?;
        if !self.alive(mate, key).await? {
            return Err(Error::Adapt("right not live".into()));
        }
        if !self.fresh(edge.target(), key).await? {
            return Err(Error::Adapt(format!("ref {} leased", edge.name())));
        }
        if edge.kind() == bond::Kind::One2one {
            self.lone(unit, edge, key, myself).await?;
        }
        Ok(Val::Int(key))
    }

    pub(super) async fn next(
        &mut self,
        unit: &Unit,
        slot: &Slot,
        scope: &str,
        fields: &[(&str, &str)],
    ) -> Result<Val, Error> {
        let hold = anchor(unit, fields, None, scope)?;
        let name = crate::estate::clock::serial(&unit.key(), slot.name(), &hold)?;
        let key = crate::estate::next(self.wire, &name).await?;
        Ok(Val::Int(key))
    }

    pub(super) async fn lone(
        &mut self,
        unit: &Unit,
        edge: &Edge,
        key: i64,
        myself: Option<i64>,
    ) -> Result<(), Error> {
        let tick = now();
        let text = format!(
            "SELECT 1 FROM {} WHERE {} = ?1 AND {} != ?2 AND ({} IS NULL OR {} > ?3) LIMIT 1",
            ddl::seat(unit),
            ddl::col(&ddl::side(edge.name())),
            ddl::KEY,
            ddl::EXPIRES,
            ddl::EXPIRES
        );
        let args = [Val::Int(key), Val::Int(myself.unwrap_or(0)), Val::Int(tick)];
        if !self.wire.rows(&text, &args).await?.is_empty() {
            return Err(Error::Adapt("live ref exists".into()));
        }
        Ok(())
    }

    pub(super) async fn entry(
        &mut self,
        unit: &Unit,
        col: &str,
        val: &str,
        myself: i64,
    ) -> Result<(String, Val), Error> {
        if let Some(edge) = unit.refs().find(|e| e.name() == col) {
            let cell = self.point(unit, edge, val, Some(myself)).await?;
            return Ok((ddl::side(edge.name()), cell));
        }
        Ok((col.to_string(), fit(unit.fields(), col, val)?))
    }

    pub(crate) async fn one(&mut self, unit: &Unit, key: i64) -> Result<Option<Row>, Error> {
        match self.peek(unit, key).await {
            Ok(row) => Ok(Some(row)),
            Err(Error::Adapt(note)) if note.starts_with("missing row") => Ok(None),
            Err(err) => Err(err),
        }
    }

    pub(super) async fn peek(&mut self, unit: &Unit, key: i64) -> Result<Row, Error> {
        let tick = now();
        let text = format!(
            "SELECT {} FROM {} WHERE {} = ?1 AND ({} IS NULL OR {} > ?2)",
            unit.sheet(),
            ddl::seat(unit),
            ddl::KEY,
            ddl::EXPIRES,
            ddl::EXPIRES
        );
        let rows = self
            .wire
            .rows(&text, &[Val::Int(key), Val::Int(tick)])
            .await?;
        match rows.first() {
            Some(line) => Row::read(unit, line),
            None => Err(Error::Adapt(format!("missing row {key}"))),
        }
    }

    pub(crate) async fn scan(&mut self, unit: &Unit) -> Result<Vec<Row>, Error> {
        let tick = now();
        let text = format!(
            "SELECT {} FROM {} WHERE {} IS NULL OR {} > ?1 ORDER BY {}",
            unit.sheet(),
            ddl::seat(unit),
            ddl::EXPIRES,
            ddl::EXPIRES,
            ddl::KEY
        );
        let mut out = Vec::new();
        for line in self.wire.rows(&text, &[Val::Int(tick)]).await? {
            out.push(Row::read(unit, &line)?);
        }
        Ok(out)
    }
}
