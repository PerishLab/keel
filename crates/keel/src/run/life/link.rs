use super::util::*;
use super::*;
use crate::adapt::Error;
use crate::ddl;
use crate::plan::{Edge, Unit};
use crate::wire::{Val, Wire};

impl<'a, W: Wire> Work<'a, W> {
    pub(crate) async fn tie(
        &mut self,
        owner: &str,
        bond: &str,
        ends: Ends,
        fields: &[(&str, &str)],
    ) -> Result<i64, Error> {
        let (unit, edge) = self.plan.edge(owner, bond)?;
        let mate = self.plan.find(edge.target())?;
        edge.part(fields)?;
        if !self.alive(unit, ends.left).await? {
            return Err(Error::Adapt("left not live".into()));
        }
        if !self.alive(mate, ends.right).await? {
            return Err(Error::Adapt("right not live".into()));
        }
        if !self.fresh(unit.name(), ends.left).await? {
            return Err(Error::Adapt("left leased".into()));
        }
        if !self.fresh(edge.target(), ends.right).await? {
            return Err(Error::Adapt("right leased".into()));
        }
        if self.paired(owner, bond, ends.left, ends.right).await? {
            return Err(Error::Adapt("live pair exists".into()));
        }
        let tick = now();
        let src = ddl::col(&ddl::side(unit.name()));
        let dst = ddl::col(&ddl::mate(unit.name(), edge.name(), edge.target()));
        let mut cols = vec![src, dst];
        for slot in edge.fields() {
            cols.push(ddl::col(slot.name()));
        }
        cols.push(ddl::EXPIRES.to_string());
        cols.push(ddl::CREATED.to_string());
        cols.push(ddl::UPDATED.to_string());
        let marks = (1..=cols.len())
            .map(|i| format!("?{i}"))
            .collect::<Vec<_>>()
            .join(", ");
        let text = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            ddl::joint(unit.name(), edge.name()),
            cols.join(", "),
            marks
        );
        let mut vals: Vec<Val> = vec![Val::Int(ends.left), Val::Int(ends.right)];
        for slot in edge.fields() {
            let hit = fields
                .iter()
                .find(|(k, _)| *k == slot.name())
                .map(|(_, v)| *v)
                .unwrap_or("");
            vals.push(slot.bind(hit)?);
        }
        vals.push(Val::Null);
        vals.push(Val::Int(tick));
        vals.push(Val::Int(tick));
        self.wire.plant(&text, &vals).await
    }

    pub(crate) async fn tune(
        &mut self,
        owner: &str,
        bond: &str,
        key: i64,
        fields: &[(&str, &str)],
    ) -> Result<(), Error> {
        let (unit, edge) = self.plan.edge(owner, bond)?;
        let mate = self.plan.find(edge.target())?;
        edge.part(fields)?;
        if fields.is_empty() {
            return Err(Error::Adapt("empty set".into()));
        }
        let ends = self.ends(owner, bond, key).await?;
        if !self.alive(unit, ends.left).await? {
            return Err(Error::Adapt("left not live".into()));
        }
        if !self.alive(mate, ends.right).await? {
            return Err(Error::Adapt("right not live".into()));
        }
        let tick = now();
        let mut text = format!("UPDATE {} SET ", ddl::joint(unit.name(), edge.name()));
        let mut vals: Vec<Val> = Vec::new();
        for (i, (col, val)) in fields.iter().enumerate() {
            if i > 0 {
                text.push_str(", ");
            }
            text.push_str(&ddl::col(col));
            text.push_str(" = ?");
            text.push_str(&(i + 1).to_string());
            vals.push(fit(edge.fields(), col, val)?);
        }
        let n = fields.len();
        text.push_str(&format!(
            ", {} = ?{} WHERE {} = ?{} AND ({} IS NULL OR {} > ?{})",
            ddl::UPDATED,
            n + 1,
            ddl::KEY,
            n + 2,
            ddl::EXPIRES,
            ddl::EXPIRES,
            n + 3
        ));
        vals.push(Val::Int(tick));
        vals.push(Val::Int(key));
        vals.push(Val::Int(tick));
        let changed = self.wire.run(&text, &vals).await?;
        if changed == 0 {
            return Err(Error::Adapt(format!("missing tie {key}")));
        }
        Ok(())
    }

    pub(super) async fn ends(&mut self, owner: &str, bond: &str, key: i64) -> Result<Ends, Error> {
        let (unit, edge) = self.plan.edge(owner, bond)?;
        let tick = now();
        let src = ddl::col(&ddl::side(unit.name()));
        let dst = ddl::col(&ddl::mate(unit.name(), edge.name(), edge.target()));
        let text = format!(
            "SELECT {}, {} FROM {} WHERE {} = ?1 AND ({} IS NULL OR {} > ?2)",
            src,
            dst,
            ddl::joint(unit.name(), edge.name()),
            ddl::KEY,
            ddl::EXPIRES,
            ddl::EXPIRES
        );
        let rows = self
            .wire
            .rows(&text, &[Val::Int(key), Val::Int(tick)])
            .await?;
        let line = rows
            .first()
            .ok_or_else(|| Error::Adapt(format!("missing tie {key}")))?;
        Ok(Ends {
            left: line[0].int(),
            right: line[1].int(),
        })
    }

    pub(crate) async fn ties(
        &mut self,
        unit: &Unit,
        edge: &Edge,
        left: i64,
    ) -> Result<Vec<Tie>, Error> {
        let tick = now();
        let src = ddl::col(&ddl::side(unit.name()));
        let dst = ddl::col(&ddl::mate(unit.name(), edge.name(), edge.target()));
        let mut cols = vec![
            ddl::KEY.to_string(),
            src,
            dst,
            ddl::EXPIRES.to_string(),
            ddl::CREATED.to_string(),
            ddl::UPDATED.to_string(),
        ];
        for slot in edge.fields() {
            cols.push(ddl::col(slot.name()));
        }
        let text = format!(
            "SELECT {} FROM {} WHERE {} = ?1 AND ({} IS NULL OR {} > ?2) ORDER BY {}",
            cols.join(", "),
            ddl::joint(unit.name(), edge.name()),
            ddl::col(&ddl::side(unit.name())),
            ddl::EXPIRES,
            ddl::EXPIRES,
            ddl::KEY
        );
        let mut out = Vec::new();
        for line in self
            .wire
            .rows(&text, &[Val::Int(left), Val::Int(tick)])
            .await?
        {
            out.push(Tie::read(edge, &line)?);
        }
        Ok(out)
    }

    pub(crate) async fn cut(&mut self, owner: &str, bond: &str, key: i64) -> Result<(), Error> {
        let (unit, edge) = self.plan.edge(owner, bond)?;
        let tick = now();
        let text = format!(
            "UPDATE {} SET {} = ?1, {} = ?1 WHERE {} = ?2",
            ddl::joint(unit.name(), edge.name()),
            ddl::EXPIRES,
            ddl::UPDATED,
            ddl::KEY
        );
        let n = self
            .wire
            .run(&text, &[Val::Int(tick), Val::Int(key)])
            .await?;
        if n == 0 {
            return Err(Error::Adapt(format!("missing tie {key}")));
        }
        Ok(())
    }

    pub(crate) async fn alive(&mut self, unit: &Unit, key: i64) -> Result<bool, Error> {
        let tick = now();
        let text = format!(
            "SELECT 1 FROM {} WHERE {} = ?1 AND ({} IS NULL OR {} > ?2) LIMIT 1",
            ddl::seat(unit.name()),
            ddl::KEY,
            ddl::EXPIRES,
            ddl::EXPIRES
        );
        let found = !self
            .wire
            .rows(&text, &[Val::Int(key), Val::Int(tick)])
            .await?
            .is_empty();
        Ok(found)
    }

    pub(super) async fn paired(
        &mut self,
        owner: &str,
        bond: &str,
        left: i64,
        right: i64,
    ) -> Result<bool, Error> {
        let (unit, edge) = self.plan.edge(owner, bond)?;
        let tick = now();
        let src = ddl::col(&ddl::side(unit.name()));
        let dst = ddl::col(&ddl::mate(unit.name(), edge.name(), edge.target()));
        let text = format!(
            "SELECT 1 FROM {} WHERE {} = ?1 AND {} = ?2 AND ({} IS NULL OR {} > ?3) LIMIT 1",
            ddl::joint(unit.name(), edge.name()),
            src,
            dst,
            ddl::EXPIRES,
            ddl::EXPIRES
        );
        let args = [Val::Int(left), Val::Int(right), Val::Int(tick)];
        let found = !self.wire.rows(&text, &args).await?.is_empty();
        Ok(found)
    }
}
