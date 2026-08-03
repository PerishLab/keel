use super::*;
use crate::adapt::Error;

pub fn form(unit: &str) -> Tree {
    Tree {
        from: unit.to_string(),
        slice: Slice::Live,
        preds: Vec::new(),
        links: Vec::new(),
        sorts: Vec::new(),
        limit: None,
        after: None,
        tally: false,
    }
}

pub fn parse(text: &str) -> Result<Tree, Error> {
    let mut scan = Scan::new(text);
    scan.ws();
    scan.kw("from")?;
    let unit = scan.ident()?;
    let mut preds = Vec::new();
    scan.ws();
    if scan.opt("where") {
        loop {
            preds.push(scan.pred()?);
            scan.ws();
            if !scan.opt("and") {
                break;
            }
        }
    }
    scan.ws();
    let tally = scan.opt("count");
    let links = scan.links()?;
    let sorts = scan.sorts()?;
    let limit = scan.limit()?;
    let after = scan.after()?;
    scan.ws();
    if !scan.done() {
        return Err(Error::Adapt("query has trailing tokens".into()));
    }
    let projection = (
        !links.is_empty(),
        !sorts.is_empty(),
        limit.is_some(),
        after.is_some(),
    );
    let more = projection != (false, false, false, false);
    if tally && more {
        return Err(Error::Adapt("count stands alone".into()));
    }
    Ok(Tree {
        from: unit,
        slice: Slice::Live,
        preds,
        links,
        sorts,
        limit,
        after,
        tally,
    })
}

pub(crate) fn edge<'a>(
    unit: &'a crate::plan::Unit,
    bond: &str,
) -> Result<&'a crate::plan::Edge, Error> {
    unit.bonds()
        .iter()
        .find(|edge| {
            edge.name().eq_ignore_ascii_case(bond) && edge.kind() == crate::bond::Kind::Many2many
        })
        .ok_or_else(|| Error::Adapt(format!("unknown bond {bond}")))
}

impl Scan<'_> {
    pub(crate) fn pred(&mut self) -> Result<Pred, Error> {
        let field = self.ident()?;
        self.ws();
        if self.opt("has") {
            let value = self.quoted()?;
            return Ok(Pred {
                field,
                op: Op::Has,
                values: vec![value],
                nest: None,
            });
        }
        if self.opt("some") {
            self.ch('(')?;
            let inner = self.ident()?;
            let nest = self.cell(inner)?;
            self.ch(')')?;
            return Ok(Pred {
                field,
                op: Op::Some,
                values: Vec::new(),
                nest: Some(Box::new(nest)),
            });
        }
        self.cell(field)
    }

    pub(crate) fn cell(&mut self, field: String) -> Result<Pred, Error> {
        self.ws();
        if self.opt("is") {
            self.kw("null")?;
            return Ok(Pred {
                field,
                op: Op::Null,
                values: Vec::new(),
                nest: None,
            });
        }
        if self.opt("in") {
            let values = self.list()?;
            return Ok(Pred {
                field,
                op: Op::In,
                values,
                nest: None,
            });
        }
        if self.opt("like") {
            self.ws();
            let value = self.quoted()?;
            return Ok(Pred {
                field,
                op: Op::Like,
                values: vec![value],
                nest: None,
            });
        }
        let op = self.op()?;
        self.ws();
        let value = self.quoted()?;
        Ok(Pred {
            field,
            op,
            values: vec![value],
            nest: None,
        })
    }

    pub(crate) fn list(&mut self) -> Result<Vec<String>, Error> {
        self.ch('(')?;
        let mut values = Vec::new();
        loop {
            values.push(self.quoted()?);
            self.ws();
            if !self.comma() {
                break;
            }
        }
        self.ch(')')?;
        if values.is_empty() {
            return Err(Error::Adapt("empty in list".into()));
        }
        Ok(values)
    }

    pub(crate) fn links(&mut self) -> Result<Vec<String>, Error> {
        let mut links = Vec::new();
        loop {
            self.ws();
            if !self.opt("link") {
                break;
            }
            let bond = self.ident()?;
            if links
                .iter()
                .any(|have: &String| have.eq_ignore_ascii_case(&bond))
            {
                return Err(Error::Adapt(format!("duplicate link {bond}")));
            }
            links.push(bond);
        }
        Ok(links)
    }

    pub(crate) fn sorts(&mut self) -> Result<Vec<Sort>, Error> {
        self.ws();
        if !self.opt("order") {
            return Ok(Vec::new());
        }
        self.kw("by")?;
        let mut sorts = Vec::new();
        loop {
            let field = self.ident()?;
            let mut rank = Rank::Asc;
            self.ws();
            if self.opt("desc") {
                rank = Rank::Desc;
            } else {
                let _ = self.opt("asc");
            }
            sorts.push(Sort { field, rank });
            self.ws();
            if !self.comma() {
                break;
            }
        }
        Ok(sorts)
    }

    pub(crate) fn limit(&mut self) -> Result<Option<usize>, Error> {
        self.ws();
        if !self.opt("limit") {
            return Ok(None);
        }
        let n = self.num()?;
        if n == 0 {
            return Err(Error::Adapt("limit must be positive".into()));
        }
        Ok(Some(n))
    }

    pub(crate) fn after(&mut self) -> Result<Option<i64>, Error> {
        self.ws();
        if !self.opt("after") {
            return Ok(None);
        }
        let text = self.quoted()?;
        let id = text
            .parse::<i64>()
            .map_err(|_| Error::Adapt("after needs integer id".into()))?;
        Ok(Some(id))
    }
}
