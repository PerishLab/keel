use crate::adapt::Error;
use crate::cap;
use crate::ddl;
use crate::life::{Ends, Row, Tie};
use crate::plan::Plan;
use crate::query::{self, Pack, Tree};
use crate::store::Store;
use std::sync::Arc;

pub struct Core<S: Store> {
    plan: Plan,
    store: S,
}

impl<S: Store> Core<S> {
    pub(crate) fn new(plan: Plan, store: S) -> Self {
        Self { plan, store }
    }

    pub fn plan(&self) -> &Plan {
        &self.plan
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    pub fn put(&self, name: &str, fields: &[(&str, &str)]) -> Result<i64, Error> {
        self.store.put(&self.plan, name, fields)
    }

    pub fn set(&self, name: &str, key: i64, fields: &[(&str, &str)]) -> Result<(), Error> {
        self.store.set(&self.plan, name, key, fields)
    }

    pub fn live(&self, name: &str) -> Result<Vec<Row>, Error> {
        let pack = self.ask(&query::form(name))?;
        Ok(pack.rows().to_vec())
    }

    pub fn query(&self, text: &str) -> Result<Pack, Error> {
        let tree = query::parse(text)?;
        self.ask(&tree)
    }

    pub fn ask(&self, tree: &Tree) -> Result<Pack, Error> {
        query::run(&self.plan, &self.store, tree)
    }

    pub fn end(&self, name: &str, key: i64) -> Result<(), Error> {
        self.store.end(&self.plan, name, key)
    }

    pub fn tie(
        &self,
        owner: &str,
        bond: &str,
        ends: Ends,
        fields: &[(&str, &str)],
    ) -> Result<i64, Error> {
        self.store.tie(&self.plan, owner, bond, ends, fields)
    }

    pub fn set_tie(
        &self,
        owner: &str,
        bond: &str,
        key: i64,
        fields: &[(&str, &str)],
    ) -> Result<(), Error> {
        self.store.set_tie(&self.plan, owner, bond, key, fields)
    }

    pub fn ties(&self, owner: &str, bond: &str, left: i64) -> Result<Vec<Tie>, Error> {
        self.store.ties(&self.plan, owner, bond, left)
    }

    pub fn cut(&self, owner: &str, bond: &str, key: i64) -> Result<(), Error> {
        self.store.cut(&self.plan, owner, bond, key)
    }

    pub fn has(&self, name: &str) -> Result<bool, Error> {
        self.store.has(name)
    }

    pub fn cols(&self, name: &str) -> Result<Vec<String>, Error> {
        self.store.cols(name)
    }

    pub fn seal(&self, token: &str) -> Result<bool, Error> {
        cap::sealed(&self.plan, &self.store, token)
    }

    pub fn share(self) -> Arc<Self> {
        Arc::new(self)
    }

    pub fn sudo(&self) -> Face<'_, S> {
        Face {
            core: self,
            who: Who::Sudo,
        }
    }

    pub fn of(&self, operator: i64) -> Face<'_, S> {
        Face {
            core: self,
            who: Who::Op(operator),
        }
    }

    pub fn anon(&self) -> Face<'_, S> {
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

pub struct Face<'a, S: Store> {
    core: &'a Core<S>,
    who: Who,
}

impl<S: Store> Face<'_, S> {
    pub fn who(&self) -> Who {
        self.who
    }

    fn free(&self) -> bool {
        matches!(self.who, Who::Sudo)
    }

    fn plan(&self) -> &Plan {
        self.core.plan()
    }

    fn seen(&self, unit: &str, key: i64) -> Result<Row, Error> {
        let row = self
            .core
            .store()
            .one(self.plan(), unit, key)?
            .ok_or_else(|| Error::Adapt(format!("missing row {key}")))?;
        let mark = cap::Mark {
            key: Some(key),
            cells: row.cells(),
        };
        if !self.held("see", unit, &mark)? {
            return Err(Error::Adapt(format!("missing row {key}")));
        }
        Ok(row)
    }

    fn held(&self, verb: &str, unit: &str, mark: &cap::Mark<'_>) -> Result<bool, Error> {
        cap::check(
            self.plan(),
            self.core.store(),
            self.who,
            verb,
            &ddl::table(unit),
            mark,
        )
    }

    fn may(&self, verb: &str, unit: &str, mark: &cap::Mark<'_>) -> Result<(), Error> {
        if self.held(verb, unit, mark)? {
            return Ok(());
        }
        Err(Error::Adapt(format!("refused {verb}")))
    }

    pub fn put(&self, name: &str, fields: &[(&str, &str)]) -> Result<i64, Error> {
        if self.free() {
            return self.core.put(name, fields);
        }
        let unit = query::resolve(self.plan(), name)?;
        if unit == cap::GRANT {
            self.narrow(fields)?;
            return self.core.put(&unit, fields);
        }
        let cells = cap::mold(self.plan(), &unit, fields);
        let mark = cap::Mark {
            key: None,
            cells: &cells,
        };
        self.may("put", &unit, &mark)?;
        let key = self.core.put(&unit, fields)?;
        self.mint(&unit, key)?;
        Ok(key)
    }

    fn mint(&self, unit: &str, key: i64) -> Result<(), Error> {
        let Who::Op(op) = self.who else {
            return Ok(());
        };
        if unit == cap::GRANT {
            return Ok(());
        }
        self.core.put(
            cap::GRANT,
            &[
                ("who", &op.to_string()),
                ("verb", "*"),
                ("unit", unit),
                ("scope", &format!("row {key}")),
            ],
        )?;
        Ok(())
    }

    fn revoke(&self, key: i64) -> Result<(), Error> {
        let row = self
            .core
            .store()
            .one(self.plan(), cap::GRANT, key)?
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
        self.narrow(&[("verb", &verb), ("unit", &unit), ("scope", &span)])?;
        self.core.end(cap::GRANT, key)
    }

    fn narrow(&self, fields: &[(&str, &str)]) -> Result<(), Error> {
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
            self.beneath(verb, &unit, span)?;
        }
        Ok(())
    }

    fn beneath(&self, verb: &str, unit: &str, span: &str) -> Result<(), Error> {
        if let Some(id) = span.strip_prefix("row ") {
            let key = id
                .parse::<i64>()
                .map_err(|_| Error::Adapt("row scope needs id".into()))?;
            let row = self
                .core
                .store()
                .one(self.plan(), unit, key)?
                .ok_or_else(|| Error::Adapt("refused put".into()))?;
            let mark = cap::Mark {
                key: Some(key),
                cells: row.cells(),
            };
            return self.may(verb, unit, &mark);
        }
        if cap::broad(
            self.plan(),
            self.core.store(),
            self.who,
            verb,
            &ddl::table(unit),
        )? {
            return Ok(());
        }
        Err(Error::Adapt("refused put".into()))
    }

    pub fn set(&self, name: &str, key: i64, fields: &[(&str, &str)]) -> Result<(), Error> {
        if self.free() {
            return self.core.set(name, key, fields);
        }
        let unit = query::resolve(self.plan(), name)?;
        let pre = self.seen(&unit, key)?;
        let mark = cap::Mark {
            key: Some(key),
            cells: pre.cells(),
        };
        self.may("set", &unit, &mark)?;
        let mut post = pre.cells().clone();
        cap::blend(self.plan(), &unit, &mut post, fields);
        let after = cap::Mark {
            key: Some(key),
            cells: &post,
        };
        self.may("set", &unit, &after)?;
        self.core.set(&unit, key, fields)
    }

    pub fn end(&self, name: &str, key: i64) -> Result<(), Error> {
        if self.free() {
            return self.core.end(name, key);
        }
        let unit = query::resolve(self.plan(), name)?;
        if unit == cap::GRANT {
            return self.revoke(key);
        }
        let row = self.seen(&unit, key)?;
        let mark = cap::Mark {
            key: Some(key),
            cells: row.cells(),
        };
        self.may("end", &unit, &mark)?;
        self.core.end(&unit, key)
    }

    pub fn live(&self, name: &str) -> Result<Vec<Row>, Error> {
        if self.free() {
            return self.core.live(name);
        }
        let unit = query::resolve(self.plan(), name)?;
        let mut rows = self.core.live(&unit)?;
        self.sift(&unit, &mut rows)?;
        Ok(rows)
    }

    fn sift(&self, unit: &str, rows: &mut Vec<Row>) -> Result<(), Error> {
        let mut keep = Vec::new();
        for row in rows.iter() {
            let mark = cap::Mark {
                key: Some(row.key()),
                cells: row.cells(),
            };
            if self.held("see", unit, &mark)? {
                keep.push(row.key());
            }
        }
        rows.retain(|row| keep.contains(&row.key()));
        Ok(())
    }

    pub fn query(&self, text: &str) -> Result<Pack, Error> {
        let tree = query::parse(text)?;
        self.ask(&tree)
    }

    pub fn ask(&self, tree: &Tree) -> Result<Pack, Error> {
        if self.free() {
            return self.core.ask(tree);
        }
        let unit = query::resolve(self.plan(), tree.from())?;
        if tree.tally() {
            let flat = query::bare(tree);
            let pack = self.core.ask(&flat)?;
            let mut rows = pack.rows().to_vec();
            self.sift(&unit, &mut rows)?;
            return Ok(Pack::tallied(ddl::table(&unit), rows.len()));
        }
        let mut pack = self.core.ask(tree)?;
        self.strain(&unit, &mut pack)?;
        Ok(pack)
    }

    fn strain(&self, unit: &str, pack: &mut Pack) -> Result<(), Error> {
        let root = ddl::table(unit);
        let mut kept: Vec<i64> = Vec::new();
        if let Some(crate::query::Bag::Unit(rows)) = pack.bags_mut().get_mut(&root) {
            self.sift(unit, rows)?;
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
                if self.spot(&target, tie.right())? {
                    hold.push(tie.key());
                }
            }
            ties.retain(|tie| hold.contains(&tie.key()));
        }
        Ok(())
    }

    fn spot(&self, unit: &str, key: i64) -> Result<bool, Error> {
        let Some(row) = self.core.store().one(self.plan(), unit, key)? else {
            return Ok(false);
        };
        let mark = cap::Mark {
            key: Some(key),
            cells: row.cells(),
        };
        self.held("see", unit, &mark)
    }

    pub fn tie(
        &self,
        owner: &str,
        bond: &str,
        ends: Ends,
        fields: &[(&str, &str)],
    ) -> Result<i64, Error> {
        if self.free() {
            return self.core.tie(owner, bond, ends, fields);
        }
        let unit = query::resolve(self.plan(), owner)?;
        let target = self.target(&unit, bond)?;
        let left = self.seen(&unit, ends.left)?;
        let mark = cap::Mark {
            key: Some(ends.left),
            cells: left.cells(),
        };
        self.may("tie", &unit, &mark)?;
        if !self.spot(&target, ends.right)? {
            return Err(Error::Adapt(format!("missing row {}", ends.right)));
        }
        self.core.tie(&unit, bond, ends, fields)
    }

    pub fn set_tie(
        &self,
        owner: &str,
        bond: &str,
        key: i64,
        fields: &[(&str, &str)],
    ) -> Result<(), Error> {
        if self.free() {
            return self.core.set_tie(owner, bond, key, fields);
        }
        let unit = query::resolve(self.plan(), owner)?;
        let tie = self.grip(&unit, bond, key)?;
        let left = self.seen(&unit, tie.left())?;
        let mark = cap::Mark {
            key: Some(tie.left()),
            cells: left.cells(),
        };
        self.may("tie", &unit, &mark)?;
        self.core.set_tie(&unit, bond, key, fields)
    }

    pub fn ties(&self, owner: &str, bond: &str, left: i64) -> Result<Vec<Tie>, Error> {
        if self.free() {
            return self.core.ties(owner, bond, left);
        }
        let unit = query::resolve(self.plan(), owner)?;
        let _ = self.seen(&unit, left)?;
        let target = self.target(&unit, bond)?;
        let ties = self.core.ties(&unit, bond, left)?;
        let mut out = Vec::new();
        for tie in ties {
            if self.spot(&target, tie.right())? {
                out.push(tie);
            }
        }
        Ok(out)
    }

    pub fn cut(&self, owner: &str, bond: &str, key: i64) -> Result<(), Error> {
        if self.free() {
            return self.core.cut(owner, bond, key);
        }
        let unit = query::resolve(self.plan(), owner)?;
        let tie = self.grip(&unit, bond, key)?;
        let left = self.seen(&unit, tie.left())?;
        let mark = cap::Mark {
            key: Some(tie.left()),
            cells: left.cells(),
        };
        self.may("cut", &unit, &mark)?;
        self.core.cut(&unit, bond, key)
    }

    fn grip(&self, unit: &str, bond: &str, key: i64) -> Result<Tie, Error> {
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
        let lefts = self.core.live(unit)?;
        for row in lefts {
            let ties = self.core.ties(unit, &name, row.key())?;
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
