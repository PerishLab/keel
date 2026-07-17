use crate::adapt::Error;
use crate::cap;
use crate::ddl;
use crate::ddl::Grain;
use crate::life::{Ends, Row, Tie, Work};
use crate::plan::Plan;
use crate::query::{self, Pack, Tree};
use crate::wire::{Val, Wire};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub const HOLD: usize = 1024;

struct Deal {
    gens: Vec<(String, i64)>,
    until: Option<i64>,
    pack: Pack,
}

#[derive(Default)]
struct Stash {
    on: bool,
    gens: Mutex<HashMap<String, i64>>,
    deals: Mutex<HashMap<String, Deal>>,
}

impl Stash {
    fn bump(&self, unit: &str) {
        if !self.on {
            return;
        }
        let owner = unit.split('.').next().unwrap_or(unit).to_string();
        if let Ok(mut gens) = self.gens.lock() {
            *gens.entry(owner).or_insert(0) += 1;
        }
    }

    fn stamp(&self, units: &[String]) -> Vec<(String, i64)> {
        let gens = self.gens.lock().ok();
        units
            .iter()
            .map(|unit| {
                let step = gens
                    .as_ref()
                    .and_then(|g| g.get(unit).copied())
                    .unwrap_or(0);
                (unit.clone(), step)
            })
            .collect()
    }

    fn read(&self, key: &str, units: &[String]) -> Option<Pack> {
        if !self.on {
            return None;
        }
        let deals = self.deals.lock().ok()?;
        let deal = deals.get(key)?;
        if deal.gens != self.stamp(units) {
            return None;
        }
        if let Some(until) = deal.until
            && crate::life::tick() >= until
        {
            return None;
        }
        Some(deal.pack.clone())
    }

    fn keep(&self, key: String, units: &[String], pack: &Pack) {
        if !self.on {
            return;
        }
        let Ok(mut deals) = self.deals.lock() else {
            return;
        };
        if deals.len() >= HOLD {
            deals.clear();
        }
        deals.insert(
            key,
            Deal {
                gens: self.stamp(units),
                until: horizon(pack),
                pack: pack.clone(),
            },
        );
    }

    fn spoil(&self) {
        if let Ok(mut deals) = self.deals.lock() {
            deals.clear();
        }
    }
}

fn horizon(pack: &Pack) -> Option<i64> {
    let mut edge: Option<i64> = None;
    for bag in pack.bags().values() {
        let ats: Vec<i64> = match bag {
            crate::query::Bag::Unit(rows) => rows.iter().filter_map(Row::expires).collect(),
            crate::query::Bag::Bond(ties) => ties.iter().filter_map(Tie::expires).collect(),
        };
        for at in ats {
            edge = Some(edge.map_or(at, |held| held.min(at)));
        }
    }
    edge
}

pub(crate) struct Seat<W: Wire> {
    wire: W,
    dirty: bool,
}

impl<W: Wire> Seat<W> {
    async fn open(&mut self) -> Result<(), Error> {
        self.wire.script("BEGIN").await?;
        self.dirty = true;
        Ok(())
    }

    async fn close(&mut self, keep: bool) -> Result<(), Error> {
        let word = if keep { "COMMIT" } else { "ROLLBACK" };
        self.wire.script(word).await?;
        self.dirty = false;
        Ok(())
    }
}

pub struct Core<W: Wire> {
    plan: Plan,
    seat: tokio::sync::Mutex<Seat<W>>,
    identity: Option<String>,
    stash: Stash,
}

impl<W: Wire> Core<W> {
    pub(crate) fn new(plan: Plan, wire: W) -> Self {
        Self {
            plan,
            seat: tokio::sync::Mutex::new(Seat { wire, dirty: false }),
            identity: None,
            stash: Stash {
                on: true,
                ..Default::default()
            },
        }
    }

    pub fn bare(mut self) -> Self {
        self.stash.on = false;
        self
    }

    pub fn identify(mut self, unit: &str) -> Result<Self, Error> {
        let name = query::resolve(&self.plan, unit)?;
        self.identity = Some(name);
        Ok(self)
    }

    pub fn identity(&self) -> Option<&str> {
        self.identity.as_deref()
    }

    pub fn plan(&self) -> &Plan {
        &self.plan
    }

    async fn seize(&self) -> Result<tokio::sync::MutexGuard<'_, Seat<W>>, Error> {
        let mut seat = self.seat.lock().await;
        if seat.dirty {
            seat.wire.script("ROLLBACK").await?;
            seat.dirty = false;
        }
        Ok(seat)
    }

    pub async fn put(&self, name: &str, fields: &[(&str, &str)]) -> Result<i64, Error> {
        self.sudo().put(name, fields).await
    }

    pub async fn set(&self, name: &str, key: i64, fields: &[(&str, &str)]) -> Result<(), Error> {
        self.sudo().set(name, key, fields).await
    }

    pub async fn live(&self, name: &str) -> Result<Vec<Row>, Error> {
        self.sudo().live(name).await
    }

    pub async fn query(&self, text: &str) -> Result<Pack, Error> {
        self.sudo().query(text).await
    }

    pub async fn ask(&self, tree: &Tree) -> Result<Pack, Error> {
        self.sudo().ask(tree).await
    }

    pub async fn end(&self, name: &str, key: i64) -> Result<(), Error> {
        self.sudo().end(name, key).await
    }

    pub async fn lease(&self, name: &str, key: i64, at: i64) -> Result<(), Error> {
        self.sudo().lease(name, key, at).await
    }

    pub async fn tie(
        &self,
        owner: &str,
        bond: &str,
        ends: Ends,
        fields: &[(&str, &str)],
    ) -> Result<i64, Error> {
        self.sudo().tie(owner, bond, ends, fields).await
    }

    pub async fn set_tie(
        &self,
        owner: &str,
        bond: &str,
        key: i64,
        fields: &[(&str, &str)],
    ) -> Result<(), Error> {
        self.sudo().set_tie(owner, bond, key, fields).await
    }

    pub async fn ties(&self, owner: &str, bond: &str, left: i64) -> Result<Vec<Tie>, Error> {
        self.sudo().ties(owner, bond, left).await
    }

    pub async fn cut(&self, owner: &str, bond: &str, key: i64) -> Result<(), Error> {
        self.sudo().cut(owner, bond, key).await
    }

    pub async fn flow(&self, cursor: i64) -> Result<Vec<Row>, Error> {
        self.sudo().flow(cursor).await
    }

    pub async fn has(&self, name: &str) -> Result<bool, Error> {
        let mut seat = self.seize().await?;
        let table = ddl::table(name);
        let rows = match seat.wire.grain() {
            Grain::Lite => {
                seat.wire
                    .rows(
                        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
                        &[Val::Text(table)],
                    )
                    .await?
            }
            Grain::Pg => {
                seat.wire
                    .rows(
                        "SELECT 1 FROM information_schema.tables WHERE table_name = ?1",
                        &[Val::Text(table)],
                    )
                    .await?
            }
        };
        Ok(!rows.is_empty())
    }

    pub async fn cols(&self, name: &str) -> Result<Vec<String>, Error> {
        let mut seat = self.seize().await?;
        let table = ddl::table(name);
        match seat.wire.grain() {
            Grain::Lite => {
                let rows = seat
                    .wire
                    .rows(&format!("PRAGMA table_info({table})"), &[])
                    .await?;
                Ok(rows.into_iter().map(|line| line[1].text()).collect())
            }
            Grain::Pg => {
                let rows = seat
                    .wire
                    .rows(
                        "SELECT column_name FROM information_schema.columns WHERE table_name = ?1 ORDER BY ordinal_position",
                        &[Val::Text(table)],
                    )
                    .await?;
                Ok(rows.into_iter().map(|line| line[0].text()).collect())
            }
        }
    }

    pub async fn seal(&self, token: &str) -> Result<bool, Error> {
        let mut seat = self.seize().await?;
        cap::sealed(&self.plan, &mut seat.wire, token).await
    }

    pub async fn batch<T>(
        &self,
        run: impl AsyncFnOnce(&mut Tx<'_, W>) -> Result<T, Error>,
    ) -> Result<T, Error> {
        self.sudo().batch(run).await
    }

    pub fn share(self) -> Arc<Self> {
        Arc::new(self)
    }

    pub fn sudo(&self) -> Face<'_, W> {
        Face {
            core: self,
            who: Who::Sudo,
        }
    }

    pub fn of(&self, operator: i64) -> Face<'_, W> {
        Face {
            core: self,
            who: Who::Op(operator),
        }
    }

    pub fn anon(&self) -> Face<'_, W> {
        Face {
            core: self,
            who: Who::Anon,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Who {
    Sudo,
    Op(i64),
    Anon,
}

pub struct Face<'a, W: Wire> {
    core: &'a Core<W>,
    who: Who,
}

impl<W: Wire> Face<'_, W> {
    pub fn who(&self) -> Who {
        self.who
    }

    async fn read<T>(
        &self,
        run: impl AsyncFnOnce(&mut Tx<'_, W>) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let mut seat = self.core.seize().await?;
        let mut tx = Tx {
            core: self.core,
            seat: &mut *seat,
            who: self.who,
        };
        run(&mut tx).await
    }

    async fn write<T>(
        &self,
        run: impl AsyncFnOnce(&mut Tx<'_, W>) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let mut seat = self.core.seize().await?;
        seat.open().await?;
        let mut tx = Tx {
            core: self.core,
            seat: &mut *seat,
            who: self.who,
        };
        let out = run(&mut tx).await;
        match out {
            Ok(value) => {
                seat.close(true).await?;
                Ok(value)
            }
            Err(err) => {
                let _ = seat.close(false).await;
                self.core.stash.spoil();
                Err(err)
            }
        }
    }

    pub async fn put(&self, name: &str, fields: &[(&str, &str)]) -> Result<i64, Error> {
        self.write(async |tx| tx.put(name, fields).await).await
    }

    pub async fn set(&self, name: &str, key: i64, fields: &[(&str, &str)]) -> Result<(), Error> {
        self.write(async |tx| tx.set(name, key, fields).await).await
    }

    pub async fn end(&self, name: &str, key: i64) -> Result<(), Error> {
        self.write(async |tx| tx.end(name, key).await).await
    }

    pub async fn lease(&self, name: &str, key: i64, at: i64) -> Result<(), Error> {
        self.write(async |tx| tx.lease(name, key, at).await).await
    }

    pub async fn tie(
        &self,
        owner: &str,
        bond: &str,
        ends: Ends,
        fields: &[(&str, &str)],
    ) -> Result<i64, Error> {
        self.write(async |tx| tx.tie(owner, bond, ends, fields).await)
            .await
    }

    pub async fn set_tie(
        &self,
        owner: &str,
        bond: &str,
        key: i64,
        fields: &[(&str, &str)],
    ) -> Result<(), Error> {
        self.write(async |tx| tx.set_tie(owner, bond, key, fields).await)
            .await
    }

    pub async fn cut(&self, owner: &str, bond: &str, key: i64) -> Result<(), Error> {
        self.write(async |tx| tx.cut(owner, bond, key).await).await
    }

    pub async fn live(&self, name: &str) -> Result<Vec<Row>, Error> {
        self.read(async |tx| tx.live(name).await).await
    }

    pub async fn query(&self, text: &str) -> Result<Pack, Error> {
        let tree = query::parse(text)?;
        self.ask(&tree).await
    }

    pub async fn ask(&self, tree: &Tree) -> Result<Pack, Error> {
        self.read(async |tx| tx.ask(tree).await).await
    }

    pub async fn ties(&self, owner: &str, bond: &str, left: i64) -> Result<Vec<Tie>, Error> {
        self.read(async |tx| tx.ties(owner, bond, left).await).await
    }

    pub async fn flow(&self, cursor: i64) -> Result<Vec<Row>, Error> {
        self.read(async |tx| tx.flow(cursor).await).await
    }

    pub async fn batch<T>(
        &self,
        run: impl AsyncFnOnce(&mut Tx<'_, W>) -> Result<T, Error>,
    ) -> Result<T, Error> {
        self.write(run).await
    }
}

pub struct Tx<'a, W: Wire> {
    core: &'a Core<W>,
    seat: &'a mut Seat<W>,
    who: Who,
}

impl<W: Wire> Tx<'_, W> {
    pub fn who(&self) -> Who {
        self.who
    }

    fn free(&self) -> bool {
        matches!(self.who, Who::Sudo)
    }

    fn plan(&self) -> &Plan {
        self.core.plan()
    }

    async fn craft(&mut self, name: &str, fields: &[(&str, &str)]) -> Result<i64, Error> {
        let plan = self.core.plan();
        let unit = query::resolve(plan, name)?;
        let key = Work::new(&mut self.seat.wire)
            .put(plan, &unit, fields)
            .await?;
        self.beat("put", &ddl::table(&unit), key).await;
        Ok(key)
    }

    async fn beat(&mut self, verb: &str, unit: &str, key: i64) {
        self.core.stash.bump(unit);
        if unit == ddl::table(cap::PULSE) || unit == ddl::table(cap::SEAL) {
            return;
        }
        let told = label(self.who);
        let plan = self.core.plan();
        if let Err(err) = Work::new(&mut self.seat.wire)
            .pulse(plan, verb, unit, key, &told)
            .await
        {
            eprintln!("keel: pulse: {err}");
        }
    }

    async fn shift(&mut self, name: &str, key: i64, fields: &[(&str, &str)]) -> Result<(), Error> {
        let plan = self.core.plan();
        let unit = query::resolve(plan, name)?;
        Work::new(&mut self.seat.wire)
            .set(plan, &unit, key, fields)
            .await?;
        self.beat("set", &ddl::table(&unit), key).await;
        Ok(())
    }

    async fn fell(&mut self, name: &str, key: i64, at: Option<i64>) -> Result<(), Error> {
        let plan = self.core.plan();
        let unit = query::resolve(plan, name)?;
        match at {
            Some(at) => {
                Work::new(&mut self.seat.wire)
                    .lease(plan, &unit, key, at)
                    .await?
            }
            None => Work::new(&mut self.seat.wire).end(plan, &unit, key).await?,
        }
        self.beat("end", &ddl::table(&unit), key).await;
        Ok(())
    }

    async fn knot(
        &mut self,
        owner: &str,
        bond: &str,
        ends: Ends,
        fields: &[(&str, &str)],
    ) -> Result<i64, Error> {
        let plan = self.core.plan();
        let unit = query::resolve(plan, owner)?;
        let key = Work::new(&mut self.seat.wire)
            .tie(plan, &unit, bond, ends, fields)
            .await?;
        self.beat("tie", &lane(&unit, bond), key).await;
        Ok(key)
    }

    async fn bend(
        &mut self,
        owner: &str,
        bond: &str,
        key: i64,
        fields: &[(&str, &str)],
    ) -> Result<(), Error> {
        let plan = self.core.plan();
        let unit = query::resolve(plan, owner)?;
        Work::new(&mut self.seat.wire)
            .set_tie(plan, &unit, bond, key, fields)
            .await?;
        self.beat("tie", &lane(&unit, bond), key).await;
        Ok(())
    }

    async fn snip(&mut self, owner: &str, bond: &str, key: i64) -> Result<(), Error> {
        let plan = self.core.plan();
        let unit = query::resolve(plan, owner)?;
        Work::new(&mut self.seat.wire)
            .cut(plan, &unit, bond, key)
            .await?;
        self.beat("cut", &lane(&unit, bond), key).await;
        Ok(())
    }

    async fn sight(&mut self, tree: &Tree) -> Result<Pack, Error> {
        let key = query::digest(tree);
        let units = query::involved(self.core.plan(), tree)?;
        if let Some(pack) = self.core.stash.read(&key, &units) {
            return Ok(pack);
        }
        let pack = query::run(self.core.plan(), &mut self.seat.wire, tree).await?;
        self.core.stash.keep(key, &units, &pack);
        Ok(pack)
    }

    async fn seen(&mut self, unit: &str, key: i64) -> Result<Row, Error> {
        let plan = self.core.plan();
        let row = Work::new(&mut self.seat.wire)
            .one(plan, unit, key)
            .await?
            .ok_or_else(|| Error::Adapt(format!("missing row {key}")))?;
        let mark = cap::Mark {
            key: Some(key),
            cells: row.cells(),
        };
        if !self.held("see", unit, &mark).await? {
            return Err(Error::Adapt(format!("missing row {key}")));
        }
        Ok(row)
    }

    async fn held(&mut self, verb: &str, unit: &str, mark: &cap::Mark<'_>) -> Result<bool, Error> {
        cap::check(
            self.core.plan(),
            &mut self.seat.wire,
            self.who,
            verb,
            &ddl::table(unit),
            mark,
        )
        .await
    }

    async fn may(&mut self, verb: &str, unit: &str, mark: &cap::Mark<'_>) -> Result<(), Error> {
        if self.held(verb, unit, mark).await? {
            return Ok(());
        }
        Err(Error::Adapt(format!("refused {verb}")))
    }

    pub async fn put(&mut self, name: &str, fields: &[(&str, &str)]) -> Result<i64, Error> {
        if self.free() {
            return self.craft(name, fields).await;
        }
        let unit = query::resolve(self.plan(), name)?;
        if unit == cap::GRANT {
            self.narrow(fields).await?;
            return self.craft(&unit, fields).await;
        }
        let cells = cap::mold(self.plan(), &unit, fields);
        let mark = cap::Mark {
            key: None,
            cells: &cells,
        };
        self.may("put", &unit, &mark).await?;
        let key = self.craft(&unit, fields).await?;
        self.mint(&unit, key).await?;
        Ok(key)
    }

    async fn mint(&mut self, unit: &str, key: i64) -> Result<(), Error> {
        if unit == cap::GRANT {
            return Ok(());
        }
        let who = match self.who {
            Who::Op(op) => op,
            Who::Anon if self.core.identity() == Some(unit) => key,
            _ => return Ok(()),
        };
        self.craft(
            cap::GRANT,
            &[
                ("who", &who.to_string()),
                ("verb", "*"),
                ("unit", unit),
                ("scope", &format!("row {key}")),
            ],
        )
        .await?;
        Ok(())
    }

    async fn revoke(&mut self, key: i64) -> Result<(), Error> {
        let plan = self.core.plan();
        let row = Work::new(&mut self.seat.wire)
            .one(plan, cap::GRANT, key)
            .await?
            .ok_or_else(|| Error::Adapt(format!("missing row {key}")))?;
        let verb = row
            .cells()
            .get("verb")
            .map(|c| c.show())
            .unwrap_or_default();
        let unit = row
            .cells()
            .get("unit")
            .map(|c| c.show())
            .unwrap_or_default();
        let span = row
            .cells()
            .get("scope")
            .map(|c| c.show())
            .unwrap_or_default();
        self.narrow(&[("verb", &verb), ("unit", &unit), ("scope", &span)])
            .await?;
        self.fell(cap::GRANT, key, None).await
    }

    async fn narrow(&mut self, fields: &[(&str, &str)]) -> Result<(), Error> {
        let verb = cap::field(fields, "verb");
        let unit = cap::field(fields, "unit");
        let span = cap::field(fields, "scope");
        if unit == "*" {
            return Err(Error::Adapt("refused put".into()));
        }
        let unit = query::resolve(self.plan(), unit)?;
        let verbs: Vec<&str> = if verb == "*" {
            cap::VERBS.to_vec()
        } else {
            vec![verb]
        };
        for verb in verbs {
            self.beneath(verb, &unit, span).await?;
        }
        Ok(())
    }

    async fn beneath(&mut self, verb: &str, unit: &str, span: &str) -> Result<(), Error> {
        if let Some(id) = span.strip_prefix("row ") {
            let key = id
                .parse::<i64>()
                .map_err(|_| Error::Adapt("row scope needs id".into()))?;
            let plan = self.core.plan();
            let row = Work::new(&mut self.seat.wire)
                .one(plan, unit, key)
                .await?
                .ok_or_else(|| Error::Adapt("refused put".into()))?;
            let mark = cap::Mark {
                key: Some(key),
                cells: row.cells(),
            };
            return self.may(verb, unit, &mark).await;
        }
        if cap::broad(
            self.core.plan(),
            &mut self.seat.wire,
            self.who,
            verb,
            &ddl::table(unit),
        )
        .await?
        {
            return Ok(());
        }
        Err(Error::Adapt("refused put".into()))
    }

    pub async fn set(
        &mut self,
        name: &str,
        key: i64,
        fields: &[(&str, &str)],
    ) -> Result<(), Error> {
        if self.free() {
            return self.shift(name, key, fields).await;
        }
        let unit = query::resolve(self.plan(), name)?;
        let pre = self.seen(&unit, key).await?;
        let mark = cap::Mark {
            key: Some(key),
            cells: pre.cells(),
        };
        self.may("set", &unit, &mark).await?;
        let mut post = pre.cells().clone();
        cap::blend(self.plan(), &unit, &mut post, fields);
        let after = cap::Mark {
            key: Some(key),
            cells: &post,
        };
        self.may("set", &unit, &after).await?;
        self.shift(&unit, key, fields).await
    }

    pub async fn end(&mut self, name: &str, key: i64) -> Result<(), Error> {
        if self.free() {
            return self.fell(name, key, None).await;
        }
        let unit = query::resolve(self.plan(), name)?;
        if unit == cap::GRANT {
            return self.revoke(key).await;
        }
        let row = self.seen(&unit, key).await?;
        let mark = cap::Mark {
            key: Some(key),
            cells: row.cells(),
        };
        self.may("end", &unit, &mark).await?;
        self.fell(&unit, key, None).await
    }

    pub async fn lease(&mut self, name: &str, key: i64, at: i64) -> Result<(), Error> {
        if self.free() {
            return self.fell(name, key, Some(at)).await;
        }
        let unit = query::resolve(self.plan(), name)?;
        let row = self.seen(&unit, key).await?;
        let mark = cap::Mark {
            key: Some(key),
            cells: row.cells(),
        };
        self.may("end", &unit, &mark).await?;
        self.fell(&unit, key, Some(at)).await
    }

    pub async fn live(&mut self, name: &str) -> Result<Vec<Row>, Error> {
        let unit = query::resolve(self.plan(), name)?;
        let pack = self.sight(&query::form(&unit)).await?;
        let mut rows = pack.rows().to_vec();
        if self.free() {
            return Ok(rows);
        }
        self.sift(&unit, &mut rows).await?;
        Ok(rows)
    }

    async fn sift(&mut self, unit: &str, rows: &mut Vec<Row>) -> Result<(), Error> {
        let mut keep = Vec::new();
        for row in rows.iter() {
            let mark = cap::Mark {
                key: Some(row.key()),
                cells: row.cells(),
            };
            if self.held("see", unit, &mark).await? {
                keep.push(row.key());
            }
        }
        rows.retain(|row| keep.contains(&row.key()));
        Ok(())
    }

    pub async fn query(&mut self, text: &str) -> Result<Pack, Error> {
        let tree = query::parse(text)?;
        self.ask(&tree).await
    }

    pub async fn ask(&mut self, tree: &Tree) -> Result<Pack, Error> {
        if self.free() {
            return self.sight(tree).await;
        }
        let unit = query::resolve(self.plan(), tree.from())?;
        if tree.tally() {
            let flat = query::bare(tree);
            let pack = self.sight(&flat).await?;
            let mut rows = pack.rows().to_vec();
            self.sift(&unit, &mut rows).await?;
            return Ok(Pack::tallied(ddl::table(&unit), rows.len()));
        }
        let mut pack = self.sight(tree).await?;
        self.strain(&unit, &mut pack).await?;
        Ok(pack)
    }

    async fn strain(&mut self, unit: &str, pack: &mut Pack) -> Result<(), Error> {
        let root = ddl::table(unit);
        let mut kept: Vec<i64> = Vec::new();
        if let Some(crate::query::Bag::Unit(rows)) = pack.bags_mut().get_mut(&root) {
            self.sift(unit, rows).await?;
            kept = rows.iter().map(Row::key).collect();
        }
        let node = self
            .plan()
            .units()
            .get(unit)
            .ok_or_else(|| Error::Missing(unit.into()))?;
        let bonds: Vec<(String, String)> = node
            .bonds()
            .iter()
            .map(|e| (format!("{root}.{}", e.name()), e.target().to_string()))
            .collect();
        for (key, target) in bonds {
            let Some(crate::query::Bag::Bond(ties)) = pack.bags_mut().get_mut(&key) else {
                continue;
            };
            let mut hold = Vec::new();
            for tie in ties.iter() {
                if !kept.contains(&tie.left()) {
                    continue;
                }
                if self.spot(&target, tie.right()).await? {
                    hold.push(tie.key());
                }
            }
            ties.retain(|tie| hold.contains(&tie.key()));
        }
        Ok(())
    }

    async fn spot(&mut self, unit: &str, key: i64) -> Result<bool, Error> {
        let plan = self.core.plan();
        let Some(row) = Work::new(&mut self.seat.wire).one(plan, unit, key).await? else {
            return Ok(false);
        };
        let mark = cap::Mark {
            key: Some(key),
            cells: row.cells(),
        };
        self.held("see", unit, &mark).await
    }

    pub async fn tie(
        &mut self,
        owner: &str,
        bond: &str,
        ends: Ends,
        fields: &[(&str, &str)],
    ) -> Result<i64, Error> {
        if self.free() {
            return self.knot(owner, bond, ends, fields).await;
        }
        let unit = query::resolve(self.plan(), owner)?;
        let target = self.target(&unit, bond)?;
        let left = self.seen(&unit, ends.left).await?;
        let mark = cap::Mark {
            key: Some(ends.left),
            cells: left.cells(),
        };
        self.may("tie", &unit, &mark).await?;
        if !self.spot(&target, ends.right).await? {
            return Err(Error::Adapt(format!("missing row {}", ends.right)));
        }
        self.knot(&unit, bond, ends, fields).await
    }

    pub async fn set_tie(
        &mut self,
        owner: &str,
        bond: &str,
        key: i64,
        fields: &[(&str, &str)],
    ) -> Result<(), Error> {
        if self.free() {
            return self.bend(owner, bond, key, fields).await;
        }
        let unit = query::resolve(self.plan(), owner)?;
        let tie = self.grip(&unit, bond, key).await?;
        let left = self.seen(&unit, tie.left()).await?;
        let mark = cap::Mark {
            key: Some(tie.left()),
            cells: left.cells(),
        };
        self.may("tie", &unit, &mark).await?;
        self.bend(&unit, bond, key, fields).await
    }

    pub async fn ties(&mut self, owner: &str, bond: &str, left: i64) -> Result<Vec<Tie>, Error> {
        let plan = self.core.plan();
        if self.free() {
            return Work::new(&mut self.seat.wire)
                .ties(plan, owner, bond, left)
                .await;
        }
        let unit = query::resolve(plan, owner)?;
        let _ = self.seen(&unit, left).await?;
        let target = self.target(&unit, bond)?;
        let ties = Work::new(&mut self.seat.wire)
            .ties(plan, &unit, bond, left)
            .await?;
        let mut out = Vec::new();
        for tie in ties {
            if self.spot(&target, tie.right()).await? {
                out.push(tie);
            }
        }
        Ok(out)
    }

    pub async fn cut(&mut self, owner: &str, bond: &str, key: i64) -> Result<(), Error> {
        if self.free() {
            return self.snip(owner, bond, key).await;
        }
        let unit = query::resolve(self.plan(), owner)?;
        let tie = self.grip(&unit, bond, key).await?;
        let left = self.seen(&unit, tie.left()).await?;
        let mark = cap::Mark {
            key: Some(tie.left()),
            cells: left.cells(),
        };
        self.may("cut", &unit, &mark).await?;
        self.snip(&unit, bond, key).await
    }

    pub async fn flow(&mut self, cursor: i64) -> Result<Vec<Row>, Error> {
        let plan = self.core.plan();
        let rows = Work::new(&mut self.seat.wire)
            .live(plan, cap::PULSE)
            .await?;
        if let Some(first) = rows.first()
            && cursor + 1 < first.key()
        {
            return Err(Error::Adapt("cursor past window".into()));
        }
        let rows: Vec<Row> = rows.into_iter().filter(|row| row.key() > cursor).collect();
        if self.free() {
            return Ok(rows);
        }
        let mut out = Vec::new();
        for row in rows {
            if self.heard(&row).await? {
                out.push(row);
            }
        }
        Ok(out)
    }

    async fn heard(&mut self, event: &Row) -> Result<bool, Error> {
        let place = event
            .cells()
            .get("unit")
            .map(crate::life::Cell::show)
            .unwrap_or_default();
        let key = match event.cells().get("key") {
            Some(crate::life::Cell::Int(key)) => *key,
            _ => return Ok(false),
        };
        if let Some((owner, bond)) = place.split_once('.') {
            return self.caught(owner, bond, key).await;
        }
        let Ok(unit) = query::resolve(self.plan(), &place) else {
            return Ok(false);
        };
        let plan = self.core.plan();
        match Work::new(&mut self.seat.wire).one(plan, &unit, key).await? {
            Some(row) => {
                let mark = cap::Mark {
                    key: Some(key),
                    cells: row.cells(),
                };
                self.held("see", &unit, &mark).await
            }
            None => {
                cap::broad(
                    self.core.plan(),
                    &mut self.seat.wire,
                    self.who,
                    "see",
                    &ddl::table(&unit),
                )
                .await
            }
        }
    }

    async fn caught(&mut self, owner: &str, bond: &str, key: i64) -> Result<bool, Error> {
        let Ok(unit) = query::resolve(self.plan(), owner) else {
            return Ok(false);
        };
        if let Ok(tie) = self.grip(&unit, bond, key).await {
            return Ok(self.seen(&unit, tie.left()).await.is_ok());
        }
        cap::broad(
            self.core.plan(),
            &mut self.seat.wire,
            self.who,
            "see",
            &ddl::table(&unit),
        )
        .await
    }

    async fn grip(&mut self, unit: &str, bond: &str, key: i64) -> Result<Tie, Error> {
        let node = self
            .plan()
            .units()
            .get(unit)
            .ok_or_else(|| Error::Missing(unit.into()))?;
        let name = node
            .bonds()
            .iter()
            .find(|e| e.name().eq_ignore_ascii_case(bond))
            .map(|e| e.name().to_string())
            .ok_or_else(|| Error::Adapt(format!("missing bond {bond}")))?;
        let pack = self.sight(&query::form(unit)).await?;
        let lefts = pack.rows().to_vec();
        for row in lefts {
            let plan = self.core.plan();
            let ties = Work::new(&mut self.seat.wire)
                .ties(plan, unit, &name, row.key())
                .await?;
            if let Some(tie) = ties.into_iter().find(|t| t.key() == key) {
                return Ok(tie);
            }
        }
        Err(Error::Adapt(format!("missing tie {key}")))
    }

    fn target(&self, unit: &str, bond: &str) -> Result<String, Error> {
        let node = self
            .plan()
            .units()
            .get(unit)
            .ok_or_else(|| Error::Missing(unit.into()))?;
        node.bonds()
            .iter()
            .find(|e| e.name().eq_ignore_ascii_case(bond))
            .map(|e| e.target().to_string())
            .ok_or_else(|| Error::Adapt(format!("missing bond {bond}")))
    }
}

fn label(who: Who) -> String {
    match who {
        Who::Sudo => "sudo".into(),
        Who::Anon => "anon".into(),
        Who::Op(id) => id.to_string(),
    }
}

fn lane(unit: &str, bond: &str) -> String {
    format!("{}.{}", ddl::table(unit), bond)
}
