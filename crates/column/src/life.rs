use crate::Error;
use crate::manifest::{self, Entry};
use crate::store::Store;
use keel::Wire;
use std::fs;
use std::io;
use std::path::Path;

const SEGMENT: u64 = 16 * 1024 * 1024;
const AGE: u64 = 5 * 60 * 1_000_000_000;
const GRACE: u64 = 10 * 60 * 1_000_000_000;

impl<W: Wire> Store<W> {
    pub async fn seal(&self, now: u64) -> Result<(), Error> {
        let mut state = self.state.lock().await;
        let due: Vec<String> = state
            .open
            .iter()
            .filter(|(_, segment)| {
                segment.bytes >= SEGMENT || now.saturating_sub(segment.born) >= AGE
            })
            .map(|(scope, _)| scope.clone())
            .collect();
        for scope in due {
            let Some(segment) = state.open.get(&scope) else {
                continue;
            };
            let path = self.part(&scope, segment.seq);
            fs::create_dir_all(
                path.parent()
                    .ok_or_else(|| Error::new("part has no directory"))?,
            )?;
            discard(&path)?;
            let summary = self.table.write(&path, segment.records.clone())?;
            let entry = Entry {
                key: 0,
                scope: scope.clone(),
                slot: summary.partition.slot,
                name: format!("{scope}/{}", segment.seq),
                since: summary.from,
                until: summary.until,
                source: segment.seq,
                tokens: segment.tokens.iter().cloned().collect(),
            };
            let entry = manifest::publish(&self.core, entry, summary.rows).await?;
            state.parts.push(entry);
            if let Some(segment) = state.open.remove(&scope) {
                segment.remove()?;
            }
        }
        Ok(())
    }

    pub async fn retain(&self, floor: u64, now: u64) -> Result<(), Error> {
        let mut state = self.state.lock().await;
        let stale: Vec<String> = state
            .open
            .iter()
            .filter(|(_, segment)| {
                segment
                    .records
                    .first()
                    .is_some_and(|record| self.table.partition(record).slot < floor)
            })
            .map(|(scope, _)| scope.clone())
            .collect();
        for scope in stale {
            if let Some(segment) = state.open.remove(&scope) {
                segment.remove()?;
            }
        }
        let (gone, kept): (Vec<Entry>, Vec<Entry>) = std::mem::take(&mut state.parts)
            .into_iter()
            .partition(|entry| entry.slot < floor);
        let keys: Vec<i64> = gone.iter().map(|entry| entry.key).collect();
        if let Err(error) = manifest::end(&self.core, &keys).await {
            state.parts = kept.into_iter().chain(gone).collect();
            return Err(error);
        }
        state.parts = kept;
        let deadline = now.saturating_add(GRACE);
        for entry in gone {
            state
                .doomed
                .push((self.part(&entry.scope, entry.source), deadline));
        }
        Ok(())
    }

    pub async fn sweep(&self, now: u64) -> Result<(), Error> {
        let mut state = self.state.lock().await;
        let (due, later): (Vec<_>, Vec<_>) = std::mem::take(&mut state.doomed)
            .into_iter()
            .partition(|(_, deadline)| *deadline <= now);
        state.doomed = later;
        for (path, _) in due {
            discard(&path)?;
            if let Some(directory) = path.parent() {
                let _ = fs::remove_dir(directory);
            }
        }
        Ok(())
    }
}

fn discard(path: &Path) -> Result<(), Error> {
    match fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error.into()),
        _ => Ok(()),
    }
}
