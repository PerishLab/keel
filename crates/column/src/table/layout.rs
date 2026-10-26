use super::Table;
use crate::Error;
use arrow_array::{RecordBatch, new_null_array};
use arrow_schema::Schema;
use std::sync::Arc;

impl Table {
    pub(crate) fn layout(&self, actual: &Schema) -> Result<(), Error> {
        let expected = self.schema();
        let width = actual.fields().len();
        if width < self.keys.len() + 2 || width > expected.fields().len() {
            return Err(Error::new(
                "part schema width is not a compatible column prefix",
            ));
        }
        if actual.metadata() != expected.metadata() {
            return Err(Error::new("part schema metadata differs from column model"));
        }
        if actual
            .fields()
            .iter()
            .zip(expected.fields())
            .any(|(left, right)| left != right)
        {
            return Err(Error::new(
                "part column name, type or nullability differs from column model",
            ));
        }
        Ok(())
    }

    pub(crate) fn align(
        &self,
        batch: &RecordBatch,
        columns: &[usize],
        physical: &[usize],
    ) -> Result<RecordBatch, Error> {
        let schema = Arc::new(self.schema().project(columns)?);
        let arrays = columns
            .iter()
            .map(|index| match physical.binary_search(index) {
                Ok(found) => batch.column(found).clone(),
                Err(_) => new_null_array(
                    schema
                        .field(columns.binary_search(index).unwrap())
                        .data_type(),
                    batch.num_rows(),
                ),
            })
            .collect();
        Ok(RecordBatch::try_new(schema, arrays)?)
    }
}
