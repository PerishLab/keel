use super::*;
use crate::adapt::Error;

pub fn form(unit: &str) -> Tree {
    Tree {
        from: unit.to_string(),
        slice: Slice::Live,
        preds: Vec::new(),
        links: Vec::new(),
        sort: None,
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
            preds.push(take_pred(&mut scan)?);
            scan.ws();
            if !scan.opt("and") {
                break;
            }
        }
    }
    scan.ws();
    let tally = scan.opt("count");
    let links = take_links(&mut scan)?;
    let sort = take_sort(&mut scan)?;
    let limit = take_limit(&mut scan)?;
    let after = take_after(&mut scan)?;
    scan.ws();
    if !scan.done() {
        return Err(Error::Adapt("query has trailing tokens".into()));
    }
    let more = !links.is_empty() || sort.is_some() || limit.is_some() || after.is_some();
    if tally && more {
        return Err(Error::Adapt("count stands alone".into()));
    }
    Ok(Tree {
        from: unit,
        slice: Slice::Live,
        preds,
        links,
        sort,
        limit,
        after,
        tally,
    })
}

pub(crate) fn take_links(scan: &mut Scan<'_>) -> Result<Vec<String>, Error> {
    let mut links = Vec::new();
    loop {
        scan.ws();
        if !scan.opt("link") {
            break;
        }
        let bond = scan.ident()?;
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

pub(crate) fn edge(unit: &crate::plan::Unit, bond: &str) -> Result<String, Error> {
    unit.bonds()
        .iter()
        .find(|edge| {
            edge.name().eq_ignore_ascii_case(bond) && edge.kind() == crate::bond::Kind::Many2many
        })
        .map(|edge| edge.name().to_string())
        .ok_or_else(|| Error::Adapt(format!("unknown bond {bond}")))
}

pub(crate) fn take_pred(scan: &mut Scan<'_>) -> Result<Pred, Error> {
    let field = scan.ident()?;
    scan.ws();
    if scan.opt("has") {
        let value = scan.quoted()?;
        return Ok(Pred {
            field,
            op: Op::Has,
            values: vec![value],
            nest: None,
        });
    }
    if scan.opt("some") {
        scan.ch('(')?;
        let nest = take_cell(scan)?;
        scan.ch(')')?;
        return Ok(Pred {
            field,
            op: Op::Some,
            values: Vec::new(),
            nest: Some(Box::new(nest)),
        });
    }
    take_cell_rest(scan, field)
}

pub(crate) fn take_cell(scan: &mut Scan<'_>) -> Result<Pred, Error> {
    let field = scan.ident()?;
    take_cell_rest(scan, field)
}

pub(crate) fn take_cell_rest(scan: &mut Scan<'_>, field: String) -> Result<Pred, Error> {
    scan.ws();
    if scan.opt("in") {
        let values = take_list(scan)?;
        return Ok(Pred {
            field,
            op: Op::In,
            values,
            nest: None,
        });
    }
    if scan.opt("like") {
        scan.ws();
        let value = scan.quoted()?;
        return Ok(Pred {
            field,
            op: Op::Like,
            values: vec![value],
            nest: None,
        });
    }
    let op = scan.op()?;
    scan.ws();
    let value = scan.quoted()?;
    Ok(Pred {
        field,
        op,
        values: vec![value],
        nest: None,
    })
}

pub(crate) fn take_list(scan: &mut Scan<'_>) -> Result<Vec<String>, Error> {
    scan.ch('(')?;
    let mut values = Vec::new();
    loop {
        values.push(scan.quoted()?);
        scan.ws();
        if !scan.comma() {
            break;
        }
    }
    scan.ch(')')?;
    if values.is_empty() {
        return Err(Error::Adapt("empty in list".into()));
    }
    Ok(values)
}

pub(crate) fn take_sort(scan: &mut Scan<'_>) -> Result<Option<Sort>, Error> {
    scan.ws();
    if !scan.opt("order") {
        return Ok(None);
    }
    scan.kw("by")?;
    let field = scan.ident()?;
    let mut rank = Rank::Asc;
    scan.ws();
    if scan.opt("desc") {
        rank = Rank::Desc;
    } else {
        let _ = scan.opt("asc");
    }
    Ok(Some(Sort { field, rank }))
}

pub(crate) fn take_limit(scan: &mut Scan<'_>) -> Result<Option<usize>, Error> {
    scan.ws();
    if !scan.opt("limit") {
        return Ok(None);
    }
    let n = scan.num()?;
    if n == 0 {
        return Err(Error::Adapt("limit must be positive".into()));
    }
    Ok(Some(n))
}

pub(crate) fn take_after(scan: &mut Scan<'_>) -> Result<Option<i64>, Error> {
    scan.ws();
    if !scan.opt("after") {
        return Ok(None);
    }
    let text = scan.quoted()?;
    let id = text
        .parse::<i64>()
        .map_err(|_| Error::Adapt("after needs integer id".into()))?;
    Ok(Some(id))
}
