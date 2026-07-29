use crate::adapt::Error;
use crate::model::manifest::{Manifest, digest};
use crate::plan::Plan;
use crate::wire::Wire;

pub(crate) mod adopt;
mod catalog;
pub(crate) mod cleanup;
pub(crate) mod clock;
mod copy;
mod evolve;
mod guard;
mod projection;
mod tables;

pub(crate) use catalog::{Catalog, bootstrap};
pub use cleanup::{Gone, Hook, Purge};
pub(crate) use clock::next;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Fault {
    Vacant,
    Token,
    Occupied,
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
            Self::Vacant => write!(f, "vacant estate needs bootstrap"),
            Self::Token => write!(f, "invalid sudo token"),
            Self::Occupied => write!(f, "occupied estate refuses bootstrap"),
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
) -> Result<(), Error> {
    if !catalog::Catalog(wire).present().await? {
        if !catalog::Catalog(wire).empty().await? {
            if !policy.adopt {
                return Err(Error::Estate(Fault::Unsealed));
            }
            adopt::run(plan, manifest, wire).await?;
        } else {
            return Err(Error::Estate(Fault::Vacant));
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
    }
    cleanup::run(policy.cleanup, manifest, wire).await?;
    Ok(())
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
    let (current, blob) = generations(wire, active).await?;
    let frame = Plan::meta();
    let lines = crate::life::Work::new(wire, &frame).glean().await?;
    let grown = crate::model::manifest::rows::Sheet(&lines)
        .gather()
        .map_err(|note| Error::Estate(Fault::Unknown(format!("schema rows {note}"))))?;
    debug_assert!(
        grown.write() == blob.write(),
        "schema rows disagree with the manifest"
    );
    let physical = catalog::Catalog(wire).shape().await?;
    if physical != held {
        return Err(Error::Estate(Fault::Drift {
            expected: digest(&held),
            found: digest(&physical),
        }));
    }
    Ok(Bound {
        generation: active,
        digest: current,
        manifest: grown,
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
