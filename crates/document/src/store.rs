use crate::carrier::{Binding, Carrier, nonce};
use crate::manifest::{DOCUMENT, ROOT};
use crate::{Error, Model, View};
use keel::{Core, Op, Wire, form};
use radicula_document::Document;
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Head {
    pub collection: String,
    pub id: i64,
    pub token: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    pub head: Head,
    pub value: Document,
}

pub struct Store<W: Wire> {
    pub(crate) core: Arc<Core<W>>,
    pub(crate) carrier: Arc<Carrier>,
    pub(crate) binding: Binding,
}

impl<W: Wire> Store<W> {
    pub async fn create(core: Arc<Core<W>>, model: Model, root: &Path) -> Result<Self, Error> {
        model.describe()?;
        let carrier = Arc::new(Carrier::create(root)?);
        let nonce = nonce()?;
        let id = core.put(ROOT, &[("nonce", nonce.as_str())]).await?;
        let binding = Binding {
            format: 1,
            root: id,
            nonce,
            model,
        };
        carrier.bind(&binding)?;
        Ok(Self {
            core,
            carrier,
            binding,
        })
    }

    pub async fn open(core: Arc<Core<W>>, model: Model, root: &Path) -> Result<Self, Error> {
        Self::resume(core, model, root, false).await
    }

    pub async fn evolve(core: Arc<Core<W>>, model: Model, root: &Path) -> Result<Self, Error> {
        Self::resume(core, model, root, true).await
    }

    async fn resume(
        core: Arc<Core<W>>,
        model: Model,
        root: &Path,
        evolve: bool,
    ) -> Result<Self, Error> {
        model.describe()?;
        let carrier = Arc::new(Carrier::open(root)?);
        let binding = carrier.binding()?;
        if binding.model != model {
            if !evolve {
                return Err(Error::Binding(
                    "model differs from the saved declaration".into(),
                ));
            }
            model.extends(&binding.model)?;
        }
        let tree = form(ROOT)
            .when("id", Op::Eq, &binding.root.to_string())
            .when("nonce", Op::Eq, &binding.nonce);
        if core.one(&tree).await?.is_none() {
            return Err(Error::Foreign(
                "native root is absent or belongs to another estate".into(),
            ));
        }
        let mut store = Self {
            core,
            carrier,
            binding,
        };
        let keep = store.inspect(&model).await?;
        if store.binding.model != model {
            store.binding.model = model;
            store.carrier.bind(&store.binding)?;
        }
        let carrier = store.carrier.clone();
        tokio::task::spawn_blocking(move || carrier.prune(&keep))
            .await
            .map_err(|error| Error::carrier(error.to_string()))??;
        Ok(store)
    }

    async fn inspect(&self, model: &Model) -> Result<BTreeSet<String>, Error> {
        let mut keep = BTreeSet::new();
        let tree = form(DOCUMENT).when("store", Op::Eq, &self.binding.root.to_string());
        let pack = self.core.ask(&tree).await?;
        for row in pack.rows() {
            let collection = row.text("collection").ok_or(Error::Syntax)?;
            let record = self.sudo().read(collection, row.key()).await?;
            model.collection(collection)?.validate(&record.value)?;
            keep.insert(format!("{}/{}", row.key(), record.head.token));
        }
        Ok(keep)
    }

    pub fn model(&self) -> &Model {
        &self.binding.model
    }

    pub fn sudo(&self) -> View<'_, W> {
        View {
            store: self,
            face: self.core.sudo(),
        }
    }

    pub fn of(&self, operator: i64) -> View<'_, W> {
        View {
            store: self,
            face: self.core.of(operator),
        }
    }

    pub fn anon(&self) -> View<'_, W> {
        View {
            store: self,
            face: self.core.anon(),
        }
    }

    pub(crate) async fn load(&self, head: &Head) -> Result<Document, Error> {
        let carrier = self.carrier.clone();
        let head = head.clone();
        let bytes = tokio::task::spawn_blocking(move || carrier.get(head.id, &head.token))
            .await
            .map_err(|error| Error::carrier(error.to_string()))??;
        let text = std::str::from_utf8(&bytes).map_err(|_| Error::Syntax)?;
        let value = Document::parse(text).map_err(Error::Document)?;
        Ok(value)
    }

    pub(crate) async fn save(&self, record: &Record) -> Result<(), Error> {
        let carrier = self.carrier.clone();
        let head = record.head.clone();
        let bytes = record.value.text().into_bytes();
        tokio::task::spawn_blocking(move || carrier.insert(head.id, &head.token, &bytes))
            .await
            .map_err(|error| Error::carrier(error.to_string()))?
    }
}
