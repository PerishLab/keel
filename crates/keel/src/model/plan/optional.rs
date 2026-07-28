use super::Unit;

impl Unit {
    pub(crate) fn check(&self, fields: &[(&str, &str)]) -> Result<(), crate::adapt::Error> {
        for slot in self.fields() {
            if slot.serial().is_some() {
                if fields.iter().any(|(key, _)| *key == slot.name()) {
                    return Err(crate::adapt::Error::Adapt(format!(
                        "serial field {}",
                        slot.name()
                    )));
                }
                continue;
            }
            if slot.need()
                && slot.rule().fallback().is_none()
                && !fields.iter().any(|(key, _)| *key == slot.name())
            {
                return Err(crate::adapt::Error::Adapt(format!(
                    "missing field {}",
                    slot.name()
                )));
            }
        }
        for (key, _) in fields {
            if !self.knows(key) {
                return Err(crate::adapt::Error::Adapt(format!("unknown field {key}")));
            }
        }
        Ok(())
    }

    pub(crate) fn loose(&self, fields: &[&str]) -> Result<(), crate::adapt::Error> {
        if fields.is_empty() {
            return Err(crate::adapt::Error::Adapt("empty unset".into()));
        }
        for name in fields {
            if let Some(slot) = self.fields().iter().find(|slot| slot.name() == *name) {
                if slot.need() || slot.serial().is_some() {
                    return Err(crate::adapt::Error::Adapt(format!("required field {name}")));
                }
                continue;
            }
            if let Some(edge) = self.refs().find(|edge| edge.name() == *name) {
                if edge.need() {
                    return Err(crate::adapt::Error::Adapt(format!("required ref {name}")));
                }
                continue;
            }
            return Err(crate::adapt::Error::Adapt(format!("unknown field {name}")));
        }
        Ok(())
    }
}
