use crate::adapt::Error;
use crate::model::manifest::{Manifest, digest};
use crate::plan::Plan;
use crate::wire::{Val, Wire};

pub(crate) mod adopt;
mod catalog;
pub(crate) mod cleanup;
pub(crate) mod clock;
mod copy;
mod evolve;
mod guard;
mod projection;
mod tables;

pub use cleanup::{Gone, Hook, Purge};
pub(crate) use clock::next;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Fault {
    Unsealed,
    Unknown(String),
    Format { found: i64, expected: i64 },
    Drift { expected: String, found: String },
    Denied { path: String, note: String },
    Blocked { path: String, check: Check },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Change {
    steps: Vec<Step>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Step {
    path: String,
    act: Act,
    check: Option<Check>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Act {
    Add,
    Drop,
    Cast,
    Alter,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Check {
    Empty,
    Clear,
    Unique,
    Presence,
    Values,
    Authority,
}

impl std::fmt::Display for Fault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsealed => write!(f, "unsealed estate"),
            Self::Unknown(note) => write!(f, "unknown estate: {note}"),
            Self::Format { found, expected } => {
                write!(f, "estate format {found} needs {expected}")
            }
            Self::Drift { expected, found } => {
                write!(f, "estate drift {expected} != {found}")
            }
            Self::Denied { path, note } => {
                write!(f, "estate delta denied at {path}: {note}")
            }
            Self::Blocked { path, check } => {
                write!(f, "estate delta blocked at {path}: {check:?}")
            }
        }
    }
}

impl Change {
    pub(crate) fn new(steps: Vec<Step>) -> Self {
        Self { steps }
    }

    pub fn steps(&self) -> &[Step] {
        &self.steps
    }
}

impl Step {
    pub(crate) fn new(path: String, act: Act, check: Option<Check>) -> Self {
        Self { path, act, check }
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn act(&self) -> Act {
        self.act
    }

    pub fn check(&self) -> Option<Check> {
        self.check
    }
}

pub(crate) async fn attach<W: Wire>(
    plan: &Plan,
    manifest: &Manifest,
    policy: adopt::Policy<'_>,
    wire: &mut W,
) -> Result<Option<String>, Error> {
    let token = if !catalog::present(wire).await? {
        if !catalog::empty(wire).await? {
            if !policy.adopt {
                return Err(Error::Estate(Fault::Unsealed));
            }
            adopt::run(plan, manifest, wire).await?;
            None
        } else {
            install(plan, manifest, wire).await?
        }
    } else {
        let bound = verify(wire).await?;
        if bound.digest != manifest.digest() {
            let change =
                crate::model::delta::plan(&bound.manifest, manifest).map_err(Error::Estate)?;
            evolve::run(
                evolve::Work {
                    plan,
                    active: &bound.manifest,
                    requested: manifest,
                    generation: bound.generation,
                    change: &change,
                },
                wire,
            )
            .await?;
        }
        None
    };
    cleanup::run(policy.cleanup, manifest, wire).await?;
    Ok(token)
}

async fn install<W: Wire>(
    plan: &Plan,
    manifest: &Manifest,
    wire: &mut W,
) -> Result<Option<String>, Error> {
    wire.script("BEGIN").await?;
    let out = seed(plan, manifest, wire).await;
    match out {
        Ok(token) => {
            wire.script("COMMIT").await?;
            Ok(token)
        }
        Err(err) => {
            let _ = wire.script("ROLLBACK").await;
            Err(err)
        }
    }
}

async fn seed<W: Wire>(
    plan: &Plan,
    manifest: &Manifest,
    wire: &mut W,
) -> Result<Option<String>, Error> {
    for stmt in crate::ddl::script(plan, wire.grain()) {
        wire.script(&stmt).await?;
    }
    for stmt in catalog::script(wire.grain()) {
        wire.script(&stmt).await?;
    }
    let generation = next(wire, "generation").await?;
    let token = crate::cap::genesis(plan, wire).await?;
    let text = manifest.write();
    wire.run(
        "INSERT INTO \"@generation\" (id, state, digest, manifest, created, retired) VALUES (?1, ?2, ?3, ?4, ?5, NULL)",
        &[
            Val::Int(generation),
            Val::Text("active".into()),
            Val::Text(manifest.digest()),
            Val::Text(text),
            Val::Int(crate::life::tick()),
        ],
    )
    .await?;
    let shape = catalog::shape(wire).await?;
    wire.run(
        "INSERT INTO \"@estate\" (id, format, active, shape) VALUES (?1, ?2, ?3, ?4)",
        &[
            Val::Int(1),
            Val::Int(catalog::FORMAT),
            Val::Int(generation),
            Val::Text(shape),
        ],
    )
    .await?;
    Ok(token)
}

struct Bound {
    generation: i64,
    digest: String,
    manifest: Manifest,
}

async fn verify<W: Wire>(wire: &mut W) -> Result<Bound, Error> {
    let roots = wire
        .rows(
            "SELECT format, active, shape FROM \"@estate\" ORDER BY id",
            &[],
        )
        .await
        .map_err(|err| Error::Estate(Fault::Unknown(format!("catalog {err}"))))?;
    if roots.len() != 1 || roots[0].len() != 3 {
        return Err(Error::Estate(Fault::Unknown(
            "catalog needs one record".into(),
        )));
    }
    let root = &roots[0];
    let found = root[0].int();
    if found != catalog::FORMAT {
        return Err(Error::Estate(Fault::Format {
            found,
            expected: catalog::FORMAT,
        }));
    }
    let active = root[1].int();
    let held = root[2].text();
    let (current, prior) = generations(wire, active).await?;
    let physical = catalog::shape(wire).await?;
    if physical != held {
        return Err(Error::Estate(Fault::Drift {
            expected: digest(&held),
            found: digest(&physical),
        }));
    }
    Ok(Bound {
        generation: active,
        digest: current,
        manifest: prior,
    })
}

async fn generations<W: Wire>(wire: &mut W, active: i64) -> Result<(String, Manifest), Error> {
    let rows = wire
        .rows(
            "SELECT id, state, digest, manifest, created, retired FROM \"@generation\" ORDER BY id",
            &[],
        )
        .await
        .map_err(|err| Error::Estate(Fault::Unknown(format!("generation {err}"))))?;
    let mut current = None;
    let mut actives = 0;
    for row in rows {
        if row.len() != 6 {
            return Err(Error::Estate(Fault::Unknown(
                "generation record shape".into(),
            )));
        }
        let state = row[1].text();
        if !matches!(state.as_str(), "active" | "candidate" | "cleanup") {
            return Err(Error::Estate(Fault::Unknown(format!("state {state}"))));
        }
        let created = row[4].int();
        let retired = row[5].opt();
        if created <= 0
            || (state == "cleanup" && retired.is_none_or(|at| at < created))
            || (state != "cleanup" && row[5].opt().is_some())
        {
            return Err(Error::Estate(Fault::Unknown("generation lifecycle".into())));
        }
        let text = row[3].text();
        let parsed = Manifest::read(&text)
            .map_err(|note| Error::Estate(Fault::Unknown(format!("manifest {note}"))))?;
        let sealed = row[2].text();
        if parsed.digest() != sealed {
            return Err(Error::Estate(Fault::Unknown(
                "manifest digest mismatch".into(),
            )));
        }
        if state == "active" {
            actives += 1;
            if row[0].int() == active {
                current = Some((sealed, parsed));
            }
        }
    }
    if actives != 1 {
        return Err(Error::Estate(Fault::Unknown(
            "catalog needs one active generation".into(),
        )));
    }
    current
        .ok_or_else(|| Error::Estate(Fault::Unknown("catalog active generation mismatch".into())))
}
