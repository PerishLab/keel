use super::{Change, Check, Fault};
use crate::adapt::Error;
use crate::model::manifest::{Atom, Bond, Limit, Manifest, Rule, Unit};
use crate::plan::Plan;
use crate::wire::{Val, Wire};

pub(super) struct Scope<'a> {
    pub(super) change: &'a Change,
    pub(super) active: &'a Manifest,
    pub(super) requested: &'a Manifest,
    pub(super) plan: &'a Plan,
}

struct Seat<'a> {
    active: &'a Unit,
    requested: Option<&'a Unit>,
}

pub(super) async fn run<W: Wire>(scope: Scope<'_>, wire: &mut W) -> Result<(), Error> {
    for step in scope.change.steps() {
        let Some(check) = step.check() else {
            continue;
        };
        if !holds(&scope, step.path(), check, wire).await? {
            return Err(Error::Estate(Fault::Blocked {
                path: step.path().into(),
                check,
            }));
        }
    }
    grants(scope.plan, wire).await
}

async fn holds<W: Wire>(
    scope: &Scope<'_>,
    path: &str,
    check: Check,
    wire: &mut W,
) -> Result<bool, Error> {
    let parts: Vec<&str> = path.split('.').collect();
    let Some(unit) = scope
        .active
        .units()
        .iter()
        .find(|unit| unit.key == parts[0])
    else {
        return Ok(true);
    };
    let seat = Seat {
        active: unit,
        requested: scope
            .requested
            .units()
            .iter()
            .find(|next| next.key == unit.key),
    };
    match check {
        Check::Empty => seat.empty(&parts, wire).await,
        Check::Clear => seat.clear(&parts, wire).await,
        Check::Unique => seat.unique(&parts, wire).await,
        Check::Presence => seat.presence(&parts, wire).await,
        Check::Values => seat.values(&parts, wire).await,
        Check::Authority => seat.authority(wire).await,
    }
}

impl Seat<'_> {
    async fn empty<W: Wire>(&self, parts: &[&str], wire: &mut W) -> Result<bool, Error> {
        let table = if parts.len() == 3 {
            crate::ddl::joiner(&self.active.table(), parts[1])
        } else {
            self.active.table()
        };
        absent(wire, &table, None).await
    }

    async fn clear<W: Wire>(&self, parts: &[&str], wire: &mut W) -> Result<bool, Error> {
        let edge = self
            .active
            .bond(parts[1])
            .ok_or_else(|| Error::Estate(Fault::Unknown(format!("bond {}", parts[1]))))?;
        if edge.kind == Bond::Many2many {
            let table = crate::ddl::joiner(&self.active.table(), &edge.name);
            return absent(wire, &table, None).await;
        }
        let col = crate::ddl::col(&crate::ddl::side(&edge.name));
        absent(
            wire,
            &self.active.table(),
            Some(&format!("{col} IS NOT NULL")),
        )
        .await
    }

    async fn unique<W: Wire>(&self, parts: &[&str], wire: &mut W) -> Result<bool, Error> {
        let target = self
            .requested
            .ok_or_else(|| Error::Estate(Fault::Unknown("requested unit".into())))?;
        if let Some(edge) = target.bond(parts[1]) {
            let col = crate::ddl::col(&crate::ddl::side(&edge.name));
            return grouped(wire, &self.active.table(), &[col]).await;
        }
        let field = target
            .fields
            .iter()
            .find(|field| field.name == parts[1])
            .ok_or_else(|| Error::Estate(Fault::Unknown(format!("field {}", parts[1]))))?;
        let mut cols = match &field.only {
            Limit::Per(scopes) => scopes
                .iter()
                .map(|name| {
                    if target.fields.iter().any(|field| field.name == *name) {
                        crate::ddl::col(name)
                    } else {
                        crate::ddl::col(&crate::ddl::side(name))
                    }
                })
                .collect(),
            _ => Vec::new(),
        };
        cols.push(crate::ddl::col(&field.name));
        grouped(wire, &self.active.table(), &cols).await
    }

    async fn presence<W: Wire>(&self, parts: &[&str], wire: &mut W) -> Result<bool, Error> {
        let requested = self
            .requested
            .ok_or_else(|| Error::Estate(Fault::Unknown("requested unit".into())))?;
        let name = if requested.fields.iter().any(|field| field.name == parts[1]) {
            parts[1].to_string()
        } else if requested.bond(parts[1]).is_some() {
            crate::ddl::side(parts[1])
        } else {
            return Err(Error::Estate(Fault::Unknown(format!("field {}", parts[1]))));
        };
        let col = crate::ddl::col(&name);
        absent(wire, &self.active.table(), Some(&format!("{col} IS NULL"))).await
    }

    async fn values<W: Wire>(&self, parts: &[&str], wire: &mut W) -> Result<bool, Error> {
        let old = self
            .active
            .fields
            .iter()
            .find(|field| field.name == parts[1])
            .ok_or_else(|| Error::Estate(Fault::Unknown(format!("field {}", parts[1]))))?;
        let new = self
            .requested
            .and_then(|unit| unit.fields.iter().find(|field| field.name == parts[1]))
            .ok_or_else(|| Error::Estate(Fault::Unknown(format!("field {}", parts[1]))))?;
        let text = format!(
            "SELECT {} FROM {} WHERE {} IS NULL OR {} > ?1",
            crate::ddl::col(&old.name),
            crate::ddl::col(&self.active.table()),
            crate::ddl::EXPIRES,
            crate::ddl::EXPIRES
        );
        let rows = wire.rows(&text, &[Val::Int(crate::life::tick())]).await?;
        Ok(rows
            .iter()
            .all(|row| valid(&row[0], old.kind, new.kind, &new.rule)))
    }

    async fn authority<W: Wire>(&self, wire: &mut W) -> Result<bool, Error> {
        let text = format!(
            "SELECT 1 FROM \"@grant\" WHERE who LIKE ?1 AND ({} IS NULL OR {} > ?2) LIMIT 1",
            crate::ddl::EXPIRES,
            crate::ddl::EXPIRES
        );
        let rows = wire
            .rows(
                &text,
                &[
                    Val::Text(format!("{} %", self.active.name)),
                    Val::Int(crate::life::tick()),
                ],
            )
            .await?;
        Ok(rows.is_empty())
    }
}

fn valid(value: &Val, active: Atom, requested: Atom, rule: &Rule) -> bool {
    if *value == Val::Null {
        return true;
    }
    let value = match (active, requested) {
        (Atom::Text, Atom::Int) => {
            let text = value.text();
            let Ok(number) = text.parse::<i64>() else {
                return false;
            };
            if number.to_string() != text {
                return false;
            }
            number.to_string()
        }
        (Atom::Text, Atom::Bool) => match value.text().as_str() {
            "true" => "true".into(),
            "false" => "false".into(),
            _ => return false,
        },
        (Atom::Int, Atom::Bool) => match value.int() {
            0 => "false".into(),
            1 => "true".into(),
            _ => return false,
        },
        (Atom::Bool, Atom::Bool | Atom::Text) => {
            if value.int() == 0 {
                "false".into()
            } else {
                "true".into()
            }
        }
        (Atom::Bool, Atom::Int) | (Atom::Int, Atom::Text) => value.int().to_string(),
        _ => value.text(),
    };
    accepts(requested, rule, &value)
}

fn accepts(kind: Atom, rule: &Rule, value: &str) -> bool {
    if !rule.values.is_empty() && !rule.values.iter().any(|item| item == value) {
        return false;
    }
    if kind == Atom::Int {
        let Ok(value) = value.parse::<i64>() else {
            return false;
        };
        if rule.min.is_some_and(|min| value < min) || rule.max.is_some_and(|max| value > max) {
            return false;
        }
    }
    true
}

async fn grants<W: Wire>(plan: &Plan, wire: &mut W) -> Result<(), Error> {
    let text = format!(
        "SELECT who, verb, unit, scope FROM \"@grant\" WHERE {} IS NULL OR {} > ?1",
        crate::ddl::EXPIRES,
        crate::ddl::EXPIRES
    );
    for row in wire.rows(&text, &[Val::Int(crate::life::tick())]).await? {
        let cells = [
            ("who", row[0].text()),
            ("verb", row[1].text()),
            ("unit", row[2].text()),
            ("scope", row[3].text()),
        ];
        let refs: Vec<(&str, &str)> = cells
            .iter()
            .map(|(key, val)| (*key, val.as_str()))
            .collect();
        let valid = crate::cap::vet(plan, &refs).and_then(|_| {
            let Some(pred) = cells[3].1.strip_prefix("pred ") else {
                return Ok(());
            };
            let tree = crate::query::parse(&format!("from {} where {pred}", cells[2].1))?;
            crate::query::analyze(plan, &tree).map(|_| ())
        });
        if valid.is_err() {
            return Err(Error::Estate(Fault::Blocked {
                path: "@grant".into(),
                check: Check::Authority,
            }));
        }
    }
    Ok(())
}

async fn grouped<W: Wire>(wire: &mut W, table: &str, cols: &[String]) -> Result<bool, Error> {
    let list = cols.join(", ");
    let value = cols
        .last()
        .ok_or_else(|| Error::Estate(Fault::Unknown("unique columns".into())))?;
    let text = format!(
        "SELECT 1 FROM {} WHERE ({} IS NULL OR {} > ?1) AND {value} IS NOT NULL GROUP BY {list} HAVING COUNT(*) > 1 LIMIT 1",
        crate::ddl::col(table),
        crate::ddl::EXPIRES,
        crate::ddl::EXPIRES
    );
    Ok(wire
        .rows(&text, &[Val::Int(crate::life::tick())])
        .await?
        .is_empty())
}

async fn absent<W: Wire>(wire: &mut W, table: &str, extra: Option<&str>) -> Result<bool, Error> {
    let extra = extra.map(|text| format!(" AND {text}")).unwrap_or_default();
    let text = format!(
        "SELECT 1 FROM {} WHERE ({} IS NULL OR {} > ?1){extra} LIMIT 1",
        crate::ddl::col(table),
        crate::ddl::EXPIRES,
        crate::ddl::EXPIRES
    );
    Ok(wire
        .rows(&text, &[Val::Int(crate::life::tick())])
        .await?
        .is_empty())
}
