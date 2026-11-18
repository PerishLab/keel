use keel_document::{Collection, Error, Failure, Field, Model, Shape};
use radicula_document::{Document, Operation, Path, Presence};

fn collection() -> Collection {
    Collection::new(
        "settings",
        Shape::Object(vec![
            Field::required("mode", Shape::Text),
            Field::optional("choice", Shape::Nullable(Box::new(Shape::Text))),
        ]),
    )
    .unwrap()
}

#[test]
fn presence() {
    let shape = collection();
    shape
        .validate(&Document::parse(r#"{"mode":"light"}"#).unwrap())
        .unwrap();
    shape
        .validate(&Document::parse(r#"{"mode":"light","choice":null}"#).unwrap())
        .unwrap();
    assert_eq!(
        shape.validate(&Document::parse(r#"{"choice":null}"#).unwrap()),
        Err(Error::Value {
            parts: vec!["mode".into()],
            failure: Failure::Missing
        })
    );
    assert_eq!(
        shape.validate(&Document::parse(r#"{"mode":null}"#).unwrap()),
        Err(Error::Value {
            parts: vec!["mode".into()],
            failure: Failure::Kind
        })
    );
}

#[test]
fn objects() {
    assert_eq!(
        collection().validate(&Document::parse(r#"{"mode":"light","extra":true}"#).unwrap()),
        Err(Error::Value {
            parts: vec!["extra".into()],
            failure: Failure::Unknown
        })
    );
    let held = Collection::new("map", Shape::Map(Box::new(Shape::Bool))).unwrap();
    held.validate(&Document::parse(r#"{"":false,"a.b":true,"a/b":false,"~":true}"#).unwrap())
        .unwrap();
    assert_eq!(
        held.validate(&Document::parse(r#"{"a.b":0}"#).unwrap()),
        Err(Error::Value {
            parts: vec!["a.b".into()],
            failure: Failure::Kind
        })
    );
}

#[test]
fn arrays() {
    let held = Collection::new(
        "list",
        Shape::Array(Box::new(Shape::Object(vec![Field::required(
            "a/b",
            Shape::Number,
        )]))),
    )
    .unwrap();
    held.validate(&Document::parse(r#"[{"a/b":0},{"a/b":1.5}]"#).unwrap())
        .unwrap();
    assert_eq!(
        held.validate(&Document::parse(r#"[{"a/b":0},{"a/b":false}]"#).unwrap()),
        Err(Error::Value {
            parts: vec!["1".into(), "a/b".into()],
            failure: Failure::Kind
        })
    );
}

#[test]
fn bindings() {
    let model = Model::new(vec![collection()]).unwrap();
    let text = model.describe().unwrap();
    assert_eq!(Model::parse(&text).unwrap(), model);
    assert!(Model::parse(&text.replace("\"format\":1", "\"format\":2")).is_err());
    assert!(Model::parse(&text.replacen('{', "{\"extra\":1,", 1)).is_err());
    assert!(model.collection("missing").is_err());
}

#[test]
fn duplicates() {
    assert!(Collection::new("", Shape::Any).is_err());
    assert!(Model::new(vec![]).is_err());
    assert!(Model::new(vec![collection(), collection()]).is_err());
    assert!(
        Collection::new(
            "duplicate",
            Shape::Object(vec![
                Field::required("same", Shape::Text),
                Field::optional("same", Shape::Number),
            ])
        )
        .is_err()
    );
}

#[test]
fn protocol() {
    let held = collection();
    let mut document = Document::parse(r#"{"mode":"light","choice":null}"#).unwrap();
    assert_eq!(
        document.read(&Path::parse("/choice").unwrap()).unwrap(),
        Presence::Present(&radicula_document::Value::Null)
    );
    document
        .apply(&[
            Operation::Remove {
                path: Path::parse("/choice").unwrap(),
            },
            Operation::Set {
                path: Path::parse("/mode").unwrap(),
                value: "dark".into(),
            },
        ])
        .unwrap();
    held.validate(&document).unwrap();
    assert_eq!(
        document.read(&Path::parse("/choice").unwrap()).unwrap(),
        Presence::Missing
    );
    let before = document.clone();
    assert!(
        document
            .apply(&[
                Operation::Set {
                    path: Path::parse("/mode").unwrap(),
                    value: "light".into()
                },
                Operation::Remove {
                    path: Path::parse("/absent").unwrap()
                },
            ])
            .is_err()
    );
    assert_eq!(document, before);
}
