use super::{Column, Table};
use crate::Error;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;

const LIMIT: usize = 1024 * 1024;
const NAME: &str = "model.json";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Model {
    format: u32,
    time: String,
    keys: Vec<String>,
    sort: Vec<String>,
    grain: Grain,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "columns"
    )]
    columns: Option<Vec<Column>>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Grain {
    #[serde(deserialize_with = "nullable")]
    key: Option<String>,
    width: u64,
}

impl Table {
    pub fn describe(&self) -> Result<String, Error> {
        self.measure()?;
        let model = Model {
            format: if self.fields.is_empty() { 1 } else { 2 },
            columns: (!self.fields.is_empty()).then(|| self.fields.clone()),
            time: self.time.clone(),
            keys: self.keys.clone(),
            sort: self
                .sort
                .iter()
                .map(|index| self.keys[*index].clone())
                .collect(),
            grain: Grain {
                key: self.grain.key.map(|index| self.keys[index].clone()),
                width: self.grain.width,
            },
        };
        let text = serde_json::to_string(&model).map_err(fault)?;
        size(text.len())?;
        Ok(text)
    }

    fn measure(&self) -> Result<(), Error> {
        let names = std::iter::once(self.time.as_str())
            .chain(self.keys.iter().map(String::as_str))
            .chain(self.sort.iter().map(|index| self.keys[*index].as_str()))
            .chain(self.grain.key.map(|index| self.keys[index].as_str()));
        let names = names.chain(self.fields.iter().map(|field| field.name.as_str()));
        let mut length = 0usize;
        for name in names {
            length = length
                .checked_add(name.len())
                .ok_or_else(|| Error::new("column model exceeds 1 MiB"))?;
            size(length)?;
        }
        Ok(())
    }

    pub fn parse(text: &str) -> Result<Self, Error> {
        size(text.len())?;
        let model: Model = serde_json::from_str(text).map_err(fault)?;
        let columns = match (model.format, model.columns) {
            (1, None) => Vec::new(),
            (2, Some(columns)) if !columns.is_empty() => columns,
            _ => return Err(Error::new("unsupported or incomplete column model format")),
        };
        let keys: Vec<_> = model.keys.iter().map(String::as_str).collect();
        let sort: Vec<_> = model.sort.iter().map(String::as_str).collect();
        let mut table = Self::new(&model.time, &keys)?
            .sort(&sort)?
            .grain(model.grain.key.as_deref(), model.grain.width)?;
        let mut names: std::collections::BTreeSet<String> = table
            .schema()
            .fields()
            .iter()
            .map(|field| field.name().clone())
            .collect();
        for column in &columns {
            if column.name.is_empty() || !column.nullable || !names.insert(column.name.clone()) {
                return Err(Error::new(
                    "nullable field declarations need distinct non-empty names and nullable=true",
                ));
            }
        }
        table.fields = columns;
        Ok(table)
    }

    pub(crate) fn saved(root: &Path) -> Result<Option<Self>, Error> {
        let mut text = String::new();
        let file = match File::open(root.join(NAME)) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        file.take((LIMIT + 1) as u64).read_to_string(&mut text)?;
        let saved = Self::parse(&text)?;
        File::open(root)?.sync_all()?;
        Ok(Some(saved))
    }

    pub(crate) fn bind(&self, root: &Path) -> Result<(), Error> {
        let text = self.describe()?;
        let mut file = tempfile::Builder::new()
            .prefix(".model-")
            .tempfile_in(root)?;
        file.write_all(text.as_bytes())?;
        file.as_file().sync_all()?;
        file.persist_noclobber(root.join(NAME))
            .map_err(|error| Error::from(error.error))?;
        File::open(root)?.sync_all()?;
        Ok(())
    }
}

pub(crate) fn occupied(root: &Path) -> Result<bool, Error> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name != "lock" && !name.starts_with(".model-") {
            return Ok(true);
        }
    }
    Ok(false)
}

fn size(length: usize) -> Result<(), Error> {
    if length > LIMIT {
        return Err(Error::new("column model exceeds 1 MiB"));
    }
    Ok(())
}

fn fault(error: serde_json::Error) -> Error {
    Error::new(format!("invalid column model: {error}"))
}

fn nullable<'de, D: serde::Deserializer<'de>>(reader: D) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(reader)
}

fn columns<'de, D: serde::Deserializer<'de>>(reader: D) -> Result<Option<Vec<Column>>, D::Error> {
    Vec::<Column>::deserialize(reader).map(Some)
}
