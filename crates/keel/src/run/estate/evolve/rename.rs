use crate::adapt::Error;
use crate::estate::Fault;
use crate::model::manifest::{Bond, Manifest, Unit};
use crate::wire::{Val, Wire};

pub(crate) struct Rename {
    pub(crate) from: String,
    pub(crate) to: String,
}

impl Rename {
    pub(crate) fn resolve(
        hints: &[(String, String)],
        active: &Manifest,
        requested: &Manifest,
    ) -> Result<Option<Self>, Error> {
        let [(from, to)] = hints else {
            return if hints.is_empty() {
                Ok(None)
            } else {
                Err(denied("rename", "exactly one rename is supported"))
            };
        };
        if from == to {
            return Err(denied(from, "source equals target"));
        }
        let target = find(requested, to).ok_or_else(|| denied(to, "unknown target"))?;
        let Some(source) = find(active, from) else {
            return if find(active, to).is_some() && active == requested {
                leaf(requested, target)?;
                Ok(None)
            } else {
                Err(denied(from, "unknown source"))
            };
        };
        if find(active, to).is_some() || find(requested, from).is_some() {
            return Err(denied(to, "source or target remains occupied"));
        }
        leaf(active, source)?;
        leaf(requested, target)?;
        let mut expected = source.clone();
        expected.key = target.key.clone();
        expected.name = target.name.clone();
        if expected != *target || !unchanged(active, requested, from, to) {
            return Err(denied(from, "rename must be the only model change"));
        }
        Ok(Some(Self {
            from: from.clone(),
            to: to.clone(),
        }))
    }

    pub(super) async fn clock<W: Wire>(&self, wire: &mut W) -> Result<(), Error> {
        let target = super::super::clock::unit(&self.to);
        if !wire
            .rows(
                "SELECT value FROM \"@clock\" WHERE name = ?1",
                &[Val::Text(target.clone())],
            )
            .await?
            .is_empty()
        {
            return Err(denied(&self.to, "historical target clock exists"));
        }
        wire.run(
            "UPDATE \"@clock\" SET name = ?1 WHERE name = ?2",
            &[
                Val::Text(target),
                Val::Text(super::super::clock::unit(&self.from)),
            ],
        )
        .await?;
        Ok(())
    }
}

fn find<'a>(manifest: &'a Manifest, key: &str) -> Option<&'a Unit> {
    manifest.units().iter().find(|unit| unit.key == key)
}

fn unchanged(active: &Manifest, requested: &Manifest, from: &str, to: &str) -> bool {
    active.units().len() == requested.units().len()
        && active
            .units()
            .iter()
            .filter(|unit| unit.key != from)
            .all(|unit| find(requested, &unit.key) == Some(unit))
        && requested
            .units()
            .iter()
            .filter(|unit| unit.key != to)
            .all(|unit| find(active, &unit.key) == Some(unit))
}

fn leaf(manifest: &Manifest, unit: &Unit) -> Result<(), Error> {
    if unit.fields.iter().any(|field| field.serial.is_some())
        || unit.bonds.iter().any(|edge| edge.kind == Bond::Many2many)
        || manifest.units().iter().any(|owner| {
            owner
                .bonds
                .iter()
                .any(|edge| edge.target.eq_ignore_ascii_case(&unit.name) || edge.target == unit.key)
        })
    {
        return Err(denied(
            &unit.key,
            "rename requires a leaf without serial or many2many",
        ));
    }
    Ok(())
}

fn denied(path: &str, note: &str) -> Error {
    Error::Estate(Fault::Denied {
        path: path.into(),
        note: note.into(),
    })
}
