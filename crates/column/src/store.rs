use crate::manifest::{self, Entry};
use crate::scan::Sink;
use crate::segment::Segment;
use crate::{Error, Filter, Record, Table, recover};
use keel::{Core, Wire};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;

const WINDOW: usize = 16;

pub struct Store<W: Wire> {
    pub(crate) core: Arc<Core<W>>,
    pub(crate) table: Table,
    pub(crate) root: PathBuf,
    pub(crate) state: Mutex<State>,
    _lock: File,
}

pub(crate) struct State {
    pub(crate) parts: Vec<Entry>,
    pub(crate) open: BTreeMap<String, Segment>,
    pub(crate) next: u64,
    pub(crate) doomed: Vec<(PathBuf, u64)>,
}

impl<W: Wire> Store<W> {
    pub async fn open(core: Arc<Core<W>>, table: Table, root: &Path) -> Result<Self, Error> {
        fs::create_dir_all(root)?;
        let lock = File::create(root.join("lock"))?;
        lock.try_lock()
            .map_err(|_| Error::new(format!("{} is held by another writer", root.display())))?;
        let parts = manifest::load(&core).await?;
        let state = recover::recover(root, parts, now())?;
        Ok(Self {
            core,
            table,
            root: root.to_path_buf(),
            state: Mutex::new(state),
            _lock: lock,
        })
    }

    pub async fn append(&self, token: &str, records: Vec<Record>) -> Result<(), Error> {
        if token.is_empty() || token.contains('\n') {
            return Err(Error::new("a batch token is one non-empty line"));
        }
        let mut grouped: BTreeMap<String, Vec<Record>> = BTreeMap::new();
        for record in records {
            self.table.check(&record)?;
            let scope = self.table.partition(&record).scope();
            grouped.entry(scope).or_default().push(record);
        }
        let mut state = self.state.lock().await;
        for (scope, batch) in grouped {
            if state.applied(&scope, token) {
                continue;
            }
            if !state.open.contains_key(&scope) {
                let seq = state.next;
                let path = located(&self.root, recover::OPEN, &scope, seq);
                state
                    .open
                    .insert(scope.clone(), Segment::create(path, seq, now())?);
                state.next += 1;
            }
            let segment = state
                .open
                .get_mut(&scope)
                .ok_or_else(|| Error::new("segment vanished"))?;
            segment.commit(token, batch)?;
        }
        Ok(())
    }

    pub async fn scan(&self, filter: &Filter, sink: &mut Sink<'_>) -> Result<(), Error> {
        let (paths, mut fresh) = {
            let state = self.state.lock().await;
            let paths: Vec<PathBuf> = state
                .parts
                .iter()
                .filter(|entry| {
                    filter.to.is_none_or(|to| entry.since < to)
                        && filter.from.is_none_or(|from| entry.until > from)
                })
                .map(|entry| self.part(&entry.scope, entry.source))
                .collect();
            let mut fresh = Vec::new();
            for record in state.open.values().flat_map(|segment| &segment.records) {
                if self.table.admits(filter, record)? {
                    fresh.push(record.clone());
                }
            }
            (paths, fresh)
        };
        for path in paths {
            self.table.scan(&path, filter, sink)?;
        }
        fresh.sort_by(|left, right| self.table.order(left, right));
        for record in fresh {
            sink(&record.raw)?;
        }
        Ok(())
    }

    pub(crate) fn part(&self, scope: &str, seq: u64) -> PathBuf {
        located(&self.root, recover::PARTS, scope, seq).with_extension("parquet")
    }
}

impl State {
    fn applied(&self, scope: &str, token: &str) -> bool {
        if self
            .open
            .get(scope)
            .is_some_and(|segment| segment.tokens.contains(token))
        {
            return true;
        }
        let mut recent: Vec<&Entry> = self
            .parts
            .iter()
            .filter(|entry| entry.scope == scope)
            .collect();
        recent.sort_by_key(|entry| std::cmp::Reverse(entry.source));
        recent
            .iter()
            .take(WINDOW)
            .any(|entry| entry.tokens.iter().any(|seen| seen == token))
    }
}

pub(crate) fn located(root: &Path, area: &str, scope: &str, seq: u64) -> PathBuf {
    root.join(area).join(scope).join(format!("{seq:020}.seg"))
}

pub(crate) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX))
        .unwrap_or_default()
}
