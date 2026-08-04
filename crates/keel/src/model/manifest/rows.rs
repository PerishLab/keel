use super::{Atom, Bond, Edge, Field, Limit, Manifest, Rule, Unit, VERSION};

pub(crate) const UNIT: &str = "@unit";
pub(crate) const BOND: &str = "@bond";
pub(crate) const FIELD: &str = "@field";
pub(crate) const SCOPE: &str = "@scope";
pub(crate) const VALUE: &str = "@value";

pub(crate) struct Line {
    pub(crate) unit: &'static str,
    pub(crate) cells: Vec<(String, String)>,
}

pub(crate) struct Sheet<'a>(pub(crate) &'a [Line]);

struct At<'a> {
    key: &'a str,
    bond: &'a str,
    name: &'a str,
}

pub(crate) fn spill(manifest: &Manifest) -> Vec<Line> {
    let mut out = Vec::new();
    for unit in &manifest.units {
        out.push(Line {
            unit: UNIT,
            cells: vec![
                ("key".into(), unit.key.clone()),
                ("name".into(), unit.name.clone()),
                ("veil".into(), flag(unit.veil)),
                ("frozen".into(), flag(unit.frozen)),
            ],
        });
        for field in &unit.fields {
            spread(&mut out, &unit.key, "", field);
        }
        for edge in &unit.bonds {
            out.push(Line {
                unit: BOND,
                cells: vec![
                    ("unit".into(), unit.key.clone()),
                    ("name".into(), edge.name.clone()),
                    ("kind".into(), link(edge.kind).into()),
                    ("target".into(), edge.target.clone()),
                    ("need".into(), flag(edge.need)),
                    ("root".into(), flag(edge.root)),
                    ("crew".into(), flag(edge.crew)),
                    ("closure".into(), flag(edge.closure)),
                ],
            });
            for field in &edge.fields {
                spread(&mut out, &unit.key, &edge.name, field);
            }
        }
    }
    out
}

fn spread(out: &mut Vec<Line>, unit: &str, bond: &str, field: &Field) {
    let mut cells = place(unit, bond, &field.name);
    cells.push(("kind".into(), sort(field.kind).into()));
    cells.push(("only".into(), bound(&field.only).into()));
    cells.push(("need".into(), flag(field.need)));
    if let Some(scope) = &field.serial {
        cells.push(("serial".into(), scope.clone()));
    }
    if let Some(value) = &field.rule.default {
        cells.push(("fallback".into(), value.clone()));
    }
    if let Some(value) = field.rule.min {
        cells.push(("min".into(), value.to_string()));
    }
    if let Some(value) = field.rule.max {
        cells.push(("max".into(), value.to_string()));
    }
    out.push(Line { unit: FIELD, cells });
    if let Limit::Per(scopes) = &field.only {
        for scope in scopes {
            let mut cells = place(unit, bond, &field.name);
            cells.push(("scope".into(), scope.clone()));
            out.push(Line { unit: SCOPE, cells });
        }
    }
    for value in &field.rule.values {
        let mut cells = place(unit, bond, &field.name);
        cells.push(("value".into(), value.clone()));
        out.push(Line { unit: VALUE, cells });
    }
}

fn place(unit: &str, bond: &str, name: &str) -> Vec<(String, String)> {
    let mut cells = vec![
        ("unit".into(), unit.to_string()),
        ("name".into(), name.to_string()),
    ];
    if !bond.is_empty() {
        cells.push(("bond".into(), bond.to_string()));
    }
    cells
}

impl Sheet<'_> {
    pub(crate) fn gather(&self) -> Result<Manifest, String> {
        let mut units = Vec::new();
        for line in self.0.iter().filter(|line| line.unit == UNIT) {
            units.push(Unit {
                key: line.pick("key")?,
                name: line.pick("name")?,
                veil: line.pick("veil")? == "true",
                frozen: line.pick("frozen")? == "true",
                fields: Vec::new(),
                bonds: Vec::new(),
            });
        }
        for unit in &mut units {
            let key = unit.key.clone();
            unit.fields = self.crop(&key, "")?;
            unit.bonds = self.tether(&key)?;
            for edge in &mut unit.bonds {
                edge.fields = self.crop(&key, &edge.name)?;
            }
            unit.fields.sort_by(|a, b| a.name.cmp(&b.name));
            unit.bonds.sort_by(|a, b| a.name.cmp(&b.name));
        }
        units.sort_by(|a, b| a.key.cmp(&b.key));
        Ok(Manifest {
            version: VERSION,
            units,
        })
    }

    fn tether(&self, key: &str) -> Result<Vec<Edge>, String> {
        let mut out = Vec::new();
        for line in self.0.iter().filter(|line| line.unit == BOND) {
            if line.pick("unit")? != key {
                continue;
            }
            out.push(Edge {
                name: line.pick("name")?,
                kind: knot(&line.pick("kind")?)?,
                target: line.pick("target")?,
                fields: Vec::new(),
                need: line.pick("need")? == "true",
                root: line.pick("root")? == "true",
                crew: line.pick("crew")? == "true",
                closure: line.pick("closure")? == "true",
            });
        }
        Ok(out)
    }

    fn crop(&self, key: &str, bond: &str) -> Result<Vec<Field>, String> {
        let mut out = Vec::new();
        for line in self.0.iter().filter(|line| line.unit == FIELD) {
            if line.pick("unit")? != key || line.look("bond") != bond {
                continue;
            }
            let name = line.pick("name")?;
            let at = At {
                key,
                bond,
                name: &name,
            };
            let only = match line.pick("only")?.as_str() {
                "free" => Limit::Free,
                "all" => Limit::All,
                "per" => Limit::Per(self.sift(&at, SCOPE)?),
                other => return Err(format!("only {other}")),
            };
            let rule = line.law(self.sift(&at, VALUE)?)?;
            out.push(Field {
                kind: grain(&line.pick("kind")?)?,
                only,
                serial: line.hold("serial"),
                need: line.pick("need")? == "true",
                rule,
                name,
            });
        }
        Ok(out)
    }

    fn sift(&self, at: &At<'_>, kind: &str) -> Result<Vec<String>, String> {
        let head = if kind == SCOPE { "scope" } else { "value" };
        let mut out = Vec::new();
        for line in self.0.iter().filter(|line| line.unit == kind) {
            if line.pick("unit")? != at.key || line.look("bond") != at.bond {
                continue;
            }
            if line.pick("name")? == at.name {
                out.push(line.pick(head)?);
            }
        }
        Ok(out)
    }
}

impl Line {
    fn law(&self, values: Vec<String>) -> Result<Rule, String> {
        Ok(Rule {
            default: self.hold("fallback"),
            values,
            min: self.span("min")?,
            max: self.span("max")?,
        })
    }

    fn span(&self, head: &str) -> Result<Option<i64>, String> {
        match self.hold(head) {
            Some(value) => value.parse().map(Some).map_err(|_| format!("{head} int")),
            None => Ok(None),
        }
    }

    pub(crate) fn pick(&self, head: &str) -> Result<String, String> {
        self.hold(head)
            .ok_or_else(|| format!("{} needs {head}", self.unit))
    }

    pub(crate) fn look(&self, head: &str) -> String {
        self.hold(head).unwrap_or_default()
    }

    fn hold(&self, head: &str) -> Option<String> {
        self.cells
            .iter()
            .find(|(name, _)| name == head)
            .map(|(_, value)| value.clone())
    }
}

fn flag(value: bool) -> String {
    if value { "true".into() } else { "false".into() }
}

fn bound(only: &Limit) -> &'static str {
    match only {
        Limit::Free => "free",
        Limit::All => "all",
        Limit::Per(_) => "per",
    }
}

fn sort(kind: Atom) -> &'static str {
    match kind {
        Atom::Text => "text",
        Atom::Link => "link",
        Atom::Int => "int",
        Atom::Bool => "bool",
    }
}

fn grain(kind: &str) -> Result<Atom, String> {
    match kind {
        "text" => Ok(Atom::Text),
        "link" => Ok(Atom::Link),
        "int" => Ok(Atom::Int),
        "bool" => Ok(Atom::Bool),
        other => Err(format!("kind {other}")),
    }
}

fn link(kind: Bond) -> &'static str {
    match kind {
        Bond::Many2many => "many2many",
        Bond::Many2one => "many2one",
        Bond::One2one => "one2one",
    }
}

fn knot(kind: &str) -> Result<Bond, String> {
    match kind {
        "many2many" => Ok(Bond::Many2many),
        "many2one" => Ok(Bond::Many2one),
        "one2one" => Ok(Bond::One2one),
        other => Err(format!("bond {other}")),
    }
}
