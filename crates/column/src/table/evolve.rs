use super::Table;
use crate::Error;
use std::fs::File;
use std::io::Write;
use std::path::Path;

impl Table {
    pub(crate) fn extends(&self, old: &Self) -> Result<(), Error> {
        let mut left = self.clone();
        let mut right = old.clone();
        left.fields.clear();
        right.fields.clear();
        if left != right || !self.fields.starts_with(&old.fields) {
            return Err(Error::new(
                "column evolution permits only an appended nullable field suffix; existing names, types, order and base layout must match",
            ));
        }
        Ok(())
    }

    pub(crate) fn evolve(&self, root: &Path, old: &Self) -> Result<(), Error> {
        self.extends(old)?;
        if Self::saved(root)?.as_ref() != Some(old) {
            return Err(Error::new("column model moved during evolution"));
        }
        let mut file = tempfile::Builder::new()
            .prefix(".model-")
            .tempfile_in(root)?;
        file.write_all(self.describe()?.as_bytes())?;
        file.as_file().sync_all()?;
        file.persist(root.join("model.json"))
            .map_err(|error| Error::from(error.error))?;
        File::open(root)?.sync_all()?;
        Ok(())
    }
}
