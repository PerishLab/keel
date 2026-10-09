use crate::Error;
use keel::atom::{int, string};
use keel::{Core, Graph, Row, Wire};

const NAME: &str = "Part";

#[keel::resource]
pub struct Part {
    #[field(string)]
    scope: string,
    #[field(int)]
    slot: int,
    #[field(string, unique)]
    name: string,
    #[field(int)]
    since: int,
    #[field(int)]
    until: int,
    #[field(int)]
    rows: int,
    #[field(int)]
    source: int,
    #[field(string, opt)]
    tokens: string,
}

pub fn stock(graph: &mut Graph) {
    graph.plug::<Part>();
}

#[derive(Clone, Debug)]
pub(crate) struct Entry {
    pub(crate) key: i64,
    pub(crate) scope: String,
    pub(crate) slot: u64,
    pub(crate) name: String,
    pub(crate) since: u64,
    pub(crate) until: u64,
    pub(crate) source: u64,
    pub(crate) tokens: Vec<String>,
}

impl Entry {
    fn read(row: &Row) -> Result<Self, Error> {
        let number = |field: &str| {
            row.int(field)
                .and_then(|value| u64::try_from(value).ok())
                .ok_or_else(|| Error::new(format!("manifest row {} lacks {field}", row.key())))
        };
        let text = |field: &str| row.text(field).unwrap_or_default().to_string();
        Ok(Self {
            key: row.key(),
            scope: text("scope"),
            slot: number("slot")?,
            name: text("name"),
            since: number("since")?,
            until: number("until")?,
            source: number("source")?,
            tokens: text("tokens").lines().map(str::to_string).collect(),
        })
    }
}

pub(crate) async fn load<W: Wire>(core: &Core<W>) -> Result<Vec<Entry>, Error> {
    core.live(NAME).await?.iter().map(Entry::read).collect()
}

pub(crate) async fn publish<W: Wire>(
    core: &Core<W>,
    mut entry: Entry,
    rows: u64,
) -> Result<Entry, Error> {
    let fields = [
        ("scope", entry.scope.clone()),
        ("slot", entry.slot.to_string()),
        ("name", entry.name.clone()),
        ("since", entry.since.to_string()),
        ("until", entry.until.to_string()),
        ("rows", rows.to_string()),
        ("source", entry.source.to_string()),
        ("tokens", entry.tokens.join("\n")),
    ];
    let pairs: Vec<(&str, &str)> = fields
        .iter()
        .map(|(name, value)| (*name, value.as_str()))
        .collect();
    entry.key = core.put(NAME, &pairs).await?;
    Ok(entry)
}

pub(crate) async fn end<W: Wire>(core: &Core<W>, keys: &[i64]) -> Result<(), Error> {
    core.batch(async |tx| {
        for key in keys {
            tx.end(NAME, *key).await?;
        }
        Ok(())
    })
    .await?;
    Ok(())
}
