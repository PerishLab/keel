use keel::adapt::Error;
use keel::{Core, Graph, Op, Row, Tree, Wire, form};
use std::fmt::Write;

pub(crate) const PARCEL: &str = "Parcel";

#[keel::resource(veil, frozen)]
struct Parcel {
    #[field(string, unique)]
    asset: keel::atom::string,
    #[field(string, unique)]
    object: keel::atom::string,
    #[field(int)]
    size: keel::atom::int,
    #[field(string)]
    hash: keel::atom::string,
}

#[keel::resource(veil, frozen)]
struct Silo {
    #[field(string, unique)]
    name: keel::atom::string,
    #[field(string)]
    address: keel::atom::string,
    #[field(string)]
    namespace: keel::atom::string,
}

pub fn catalog(graph: &mut Graph) {
    graph.plug::<Silo>().plug::<Parcel>();
}

pub(crate) struct Binding {
    pub(crate) asset: String,
    pub(crate) object: String,
    pub(crate) size: i64,
    pub(crate) hash: String,
}

impl Binding {
    pub(crate) async fn record<W: Wire>(&self, core: &Core<W>) -> Result<(), Error> {
        core.put(
            PARCEL,
            &[
                ("asset", &self.asset),
                ("object", &self.object),
                ("size", &self.size.to_string()),
                ("hash", &self.hash),
            ],
        )
        .await?;
        Ok(())
    }

    pub(crate) fn read(row: &Row) -> Result<Self, Error> {
        Ok(Self {
            asset: text(row, "asset")?.into(),
            object: text(row, "object")?.into(),
            size: row.int("size").ok_or_else(shape)?,
            hash: text(row, "hash")?.into(),
        })
    }

    pub(crate) fn matches(&self, row: &Row, prefix: &str) -> bool {
        self.object.starts_with(prefix)
            && row.int("size") == Some(self.size)
            && row.text("hash") == Some(self.hash.as_str())
    }
}

pub(crate) async fn prefix<W: Wire>(core: &Core<W>, address: &str) -> Result<String, Error> {
    if !core.plan().veiled(PARCEL) || !core.plan().veiled("Silo") {
        return Err(shape());
    }
    let query = by("Silo", "name", "asset");
    let row = match core.one(&query).await? {
        Some(row) => row,
        None => {
            let namespace = nonce()?;
            let put = core
                .put(
                    "Silo",
                    &[
                        ("name", "asset"),
                        ("address", address),
                        ("namespace", &namespace),
                    ],
                )
                .await;
            match core.one(&query).await? {
                Some(row) => row,
                None => return Err(put.err().unwrap_or_else(shape)),
            }
        }
    };
    if text(&row, "address")? != address {
        return Err(Error::Adapt("asset vault belongs to another bucket".into()));
    }
    let name = text(&row, "namespace")?;
    if name.len() != 32 || !name.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(shape());
    }
    Ok(format!("asset/{name}/"))
}

pub(crate) fn by(unit: &str, field: &str, value: &str) -> Tree {
    form(unit).when(field, Op::Eq, value)
}

pub(crate) fn nonce() -> Result<String, Error> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| Error::Adapt("asset randomness unavailable".into()))?;
    let mut name = String::with_capacity(32);
    for byte in bytes {
        write!(name, "{byte:02x}").expect("writing to a string");
    }
    Ok(name)
}

fn text<'a>(row: &'a Row, field: &str) -> Result<&'a str, Error> {
    row.text(field).ok_or_else(shape)
}

fn shape() -> Error {
    Error::Adapt("invalid asset catalog".into())
}
