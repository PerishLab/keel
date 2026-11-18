#![allow(dead_code)]

use keel::adapt::db::Sqlite;
use keel::{Core, Graph, bind, bootstrap};
use keel_document::{Collection, Model, Shape};
use radicula_document::{Document, Operation, Path};
use std::path::Path as Directory;
use std::sync::Arc;

pub fn model() -> Model {
    Model::new(vec![Collection::new("settings", Shape::Any).unwrap()]).unwrap()
}

pub async fn core(home: &Directory) -> Arc<Core<Sqlite>> {
    let file = home.join("estate.sqlite3");
    let fresh = !file.exists();
    let wire = Sqlite::file(&file).await.unwrap();
    let mut graph = Graph::new();
    keel_document::stock(&mut graph);
    graph.add(
        keel::spec::Spec::build("Actor")
            .sole("login", keel::atom::Kind::Text)
            .seal(),
    );
    let core = if fresh {
        let mut boot = bootstrap(graph, wire).unwrap();
        let token = boot.mint().await.unwrap();
        boot.seal(&token).await.unwrap()
    } else {
        bind(graph, wire).await.unwrap()
    };
    Arc::new(core.identify("Actor").unwrap())
}

pub fn document(text: &str) -> Document {
    Document::parse(text).unwrap()
}

pub fn set(path: &str, text: &str) -> Operation {
    Operation::Set {
        path: Path::parse(path).unwrap(),
        value: document(text).take(),
    }
}
