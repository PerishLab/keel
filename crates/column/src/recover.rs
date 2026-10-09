use crate::Error;
use crate::manifest::Entry;
use crate::segment::Segment;
use crate::store::State;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub(crate) const OPEN: &str = "open";
pub(crate) const PARTS: &str = "parts";

pub(crate) fn recover(root: &Path, parts: Vec<Entry>, born: u64) -> Result<State, Error> {
    let published: BTreeSet<(String, u64)> = parts
        .iter()
        .map(|entry| (entry.scope.clone(), entry.source))
        .collect();
    let mut next = parts
        .iter()
        .map(|entry| entry.source + 1)
        .max()
        .unwrap_or(1);
    let mut open = BTreeMap::new();
    for (scope, seq, path) in listed(&root.join(OPEN))? {
        let seq = seq.ok_or_else(|| Error::new(format!("unknown file {}", path.display())))?;
        next = next.max(seq + 1);
        if published.contains(&(scope.clone(), seq)) {
            fs::remove_file(&path)?;
            continue;
        }
        let segment = Segment::load(path, seq, born)?;
        if segment.records.is_empty() {
            segment.remove()?;
        } else if let Some(other) = open.insert(scope, segment) {
            return Err(Error::new(format!(
                "two open segments in one partition: {}",
                other.path().display()
            )));
        }
    }
    for (scope, seq, path) in listed(&root.join(PARTS))? {
        let kept = seq.is_some_and(|seq| {
            path.extension()
                .is_some_and(|extension| extension == "parquet")
                && published.contains(&(scope, seq))
        });
        if !kept {
            fs::remove_file(&path)?;
        }
        if let Some(seq) = seq {
            next = next.max(seq + 1);
        }
    }
    Ok(State {
        parts,
        open,
        next,
        doomed: Vec::new(),
    })
}

fn listed(area: &Path) -> Result<Vec<(String, Option<u64>, PathBuf)>, Error> {
    let mut found = Vec::new();
    for scope in entries(area)? {
        let name = scope
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();
        for path in entries(&scope)? {
            let seq = path
                .file_name()
                .and_then(|file| file.to_str())
                .and_then(|file| file.split('.').next())
                .and_then(|stem| stem.parse().ok());
            found.push((name.clone(), seq, path));
        }
    }
    Ok(found)
}

fn entries(directory: &Path) -> Result<Vec<PathBuf>, Error> {
    match fs::read_dir(directory) {
        Ok(listing) => Ok(listing
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<Result<Vec<_>, _>>()?),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error.into()),
    }
}
