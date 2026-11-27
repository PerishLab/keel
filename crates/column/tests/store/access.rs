use super::support;

use keel::adapt::db::Sqlite;
use keel::atom::string;
use keel::{Core, Graph, bind, bootstrap};
use keel_column::{Channel, Filter, Record, Store};
use std::path::Path;
use std::sync::Arc;

#[keel::resource]
struct Actor {
    #[field(string)]
    name: string,
}

async fn open(home: &Path) -> (Arc<Core<Sqlite>>, Store<Sqlite>) {
    let mut graph = Graph::new();
    keel_column::stock(&mut graph);
    graph.plug::<Actor>().plug::<Channel>();
    let path = home.join("estate");
    let fresh = !path.exists();
    let wire = Sqlite::file(&path).await.unwrap();
    let core = if fresh {
        let mut boot = bootstrap(graph, wire).unwrap();
        let key = boot.mint().await.unwrap();
        boot.seal(&key).await.unwrap()
    } else {
        bind(graph, wire).await.unwrap()
    };
    let core = Arc::new(core);
    let store = Store::open(core.clone(), support::table(), &home.join("column"))
        .await
        .unwrap();
    (core, store)
}

async fn permit(core: &Core<Sqlite>, actor: i64, mode: &str) -> i64 {
    let row = core
        .put("Channel", &[("source", "split"), ("mode", mode)])
        .await
        .unwrap();
    core.grant(keel::Grant::new(
        &actor.to_string(),
        "see",
        "Channel",
        &format!("row {row}"),
    ))
    .await
    .unwrap()
}

fn record(source: &str, raw: &[u8]) -> Record {
    Record {
        raw: raw.to_vec(),
        time: 7,
        keys: vec![source.to_owned(), "trace".to_owned()],
    }
}

async fn read(
    store: &Store<Sqlite>,
    who: i64,
    source: &str,
) -> Result<Vec<Vec<u8>>, keel_column::Error> {
    let mut result = Vec::new();
    store
        .of(who, source)
        .scan(&Filter::default(), &mut |raw| {
            result.push(raw.to_vec());
            Ok(())
        })
        .await?;
    Ok(result)
}

#[tokio::test]
async fn scoped() {
    let dir = tempfile::tempdir().unwrap();
    let (core, store) = open(dir.path()).await;
    let writer = core.put("Actor", &[("name", "writer")]).await.unwrap();
    let reader = core.put("Actor", &[("name", "reader")]).await.unwrap();
    let grant = permit(&core, writer, "append").await;
    permit(&core, reader, "scan").await;
    store
        .append("local", vec![record("pixel", b"private")])
        .await
        .unwrap();
    let view = store.of(writer, "split");
    let mixed = vec![record("split", b"first"), record("pixel", b"second")];
    assert!(view.append("mixed", mixed).await.unwrap_err().refused());
    assert!(read(&store, reader, "split").await.unwrap().is_empty());
    assert!(
        store
            .of(writer, "pixel")
            .append("wrong", vec![record("pixel", b"wrong")])
            .await
            .is_err()
    );
    assert!(
        store
            .of(reader, "split")
            .append("wrong", vec![record("split", b"wrong")])
            .await
            .is_err()
    );
    let raw = b"sample\0\xff";
    view.append("device", vec![record("split", raw)])
        .await
        .unwrap();
    assert!(read(&store, writer, "split").await.unwrap_err().refused());
    assert!(read(&store, reader, "pixel").await.is_err());
    assert_eq!(read(&store, reader, "split").await.unwrap(), [raw]);
    store.seal(u64::MAX).await.unwrap();
    drop(store);
    drop(core);
    let (core, store) = open(dir.path()).await;
    let view = store.of(writer, "split");
    view.append("device", vec![record("split", raw)])
        .await
        .unwrap();
    assert_eq!(read(&store, reader, "split").await.unwrap(), [raw]);
    core.revoke(grant).await.unwrap();
    assert!(
        view.append("device", vec![record("split", raw)])
            .await
            .is_err()
    );
    assert!(
        view.append("new", vec![record("split", b"new")])
            .await
            .is_err()
    );
    assert_eq!(read(&store, reader, "split").await.unwrap(), [raw]);
}

#[tokio::test]
async fn queried() {
    let dir = tempfile::tempdir().unwrap();
    let (core, store) = open(dir.path()).await;
    let reader = core.put("Actor", &[("name", "reader")]).await.unwrap();
    store
        .append(
            "one",
            vec![record("split", b"first"), record("pixel", b"second")],
        )
        .await
        .unwrap();
    assert!(
        store
            .of(reader, "split")
            .query(&keel_column::Query::count(Filter::default()), &mut |_| {
                unreachable!()
            })
            .await
            .unwrap_err()
            .refused()
    );
    permit(&core, reader, "scan").await;
    let mut found = Vec::new();
    store
        .of(reader, "split")
        .query(
            &keel_column::Query::select(Filter::default(), &["raw"]),
            &mut |batch| {
                let raw = batch
                    .column(0)
                    .as_any()
                    .downcast_ref::<arrow_array::BinaryArray>()
                    .unwrap();
                for index in 0..batch.num_rows() {
                    found.push(raw.value(index).to_vec());
                }
                Ok(())
            },
        )
        .await
        .unwrap();
    assert_eq!(found, [b"first".to_vec()]);
    let foreign = Filter {
        keys: vec![("producer".into(), "pixel".into())],
        ..Filter::default()
    };
    assert!(
        store
            .of(reader, "split")
            .query(&keel_column::Query::count(foreign), &mut |_| {
                unreachable!()
            })
            .await
            .unwrap_err()
            .refused()
    );
}

#[tokio::test]
async fn joined() {
    let dir = tempfile::tempdir().unwrap();
    let (core, store) = open(dir.path()).await;
    let reader = core.put("Actor", &[("name", "reader")]).await.unwrap();
    permit(&core, reader, "scan").await;
    let mut split = Vec::new();
    let mut pixel = Vec::new();
    for trace in ["trace-1", "trace-2"] {
        split.push(Record {
            raw: format!("split-{trace}").into_bytes(),
            time: 7,
            keys: vec!["split".into(), trace.into()],
        });
        pixel.push(Record {
            raw: format!("pixel-{trace}").into_bytes(),
            time: 7,
            keys: vec!["pixel".into(), trace.into()],
        });
    }
    store.append("one", split).await.unwrap();
    store.append("two", pixel).await.unwrap();
    let side = || keel_column::join::Side::new(Filter::default(), "trace", &["raw"]);
    let join = keel_column::join::Query::new(side(), side());
    let mut pairs = 0;
    store
        .of(reader, "split")
        .join(&join, &mut |batch| {
            pairs += batch.left.num_rows();
            Ok(())
        })
        .await
        .unwrap();
    assert_eq!(pairs, 2);
    let mut bare = 0;
    store
        .join(&join, &mut |batch| {
            bare += batch.left.num_rows();
            Ok(())
        })
        .await
        .unwrap();
    assert_eq!(bare, 8);
    let mut crossed = keel_column::join::Side::new(Filter::default(), "trace", &["raw"]);
    crossed.when = vec![keel_column::join::Condition::Equal(
        "producer".into(),
        keel_column::Value::Text("pixel".into()),
    )];
    let join = keel_column::join::Query::new(crossed, side());
    assert!(
        store
            .of(reader, "split")
            .join(&join, &mut |_| unreachable!())
            .await
            .unwrap_err()
            .refused()
    );
}

#[tokio::test]
async fn legacy() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("column");
    let path = dir.path().join("estate");
    let mut graph = Graph::new();
    keel_column::stock(&mut graph);
    let mut boot = bootstrap(graph, Sqlite::file(&path).await.unwrap()).unwrap();
    let key = boot.mint().await.unwrap();
    let core = Arc::new(boot.seal(&key).await.unwrap());
    let store = Store::open(core, support::table(), &root).await.unwrap();
    store
        .append("local", vec![record("split", b"old")])
        .await
        .unwrap();
    assert!(read(&store, 1, "split").await.is_err());
    drop(store);
    let (core, store) = open(dir.path()).await;
    let reader = core.put("Actor", &[("name", "reader")]).await.unwrap();
    permit(&core, reader, "scan").await;
    assert_eq!(read(&store, reader, "split").await.unwrap(), [b"old"]);
    let unkeyed = Store::open(
        core,
        keel_column::Table::new("at", &["producer"]).unwrap(),
        &dir.path().join("unkeyed"),
    )
    .await
    .unwrap();
    assert!(!read(&unkeyed, reader, "split").await.unwrap_err().refused());
}
