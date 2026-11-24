use keel_column::join::{Condition, Mode, Query as Join, Side};
use keel_column::{Filter, Limit, Projection, Query, Value};

fn scan() -> Query {
    let mut query = Query::select(
        Filter {
            from: Some(2),
            to: Some(9),
            keys: vec![("trace".into(), "trace-2".into())],
        },
        &["raw", "trace", "at"],
    );
    query.batch = 2;
    query
}

#[test]
fn roundtrip() {
    let query = scan();
    let text = query.describe().unwrap();
    assert_eq!(
        text,
        r#"{"format":1,"kind":"scan","filter":{"from":2,"to":9,"keys":[{"name":"trace","value":"trace-2"}]},"projection":{"columns":["raw","trace","at"]},"batch":2,"limit":{"candidates":10000000,"bytes":268435456,"rows":1000000,"memory":268435456}}"#
    );
    assert_eq!(Query::parse(&text).unwrap(), query);
}

#[test]
fn counted() {
    let query = Query::count(Filter::default());
    let text = query.describe().unwrap();
    assert!(text.contains(r#""projection":{"count":true}"#));
    assert_eq!(Query::parse(&text).unwrap(), query);
}

#[test]
fn defaults() {
    let query = Query::select(Filter::default(), &["at"]);
    let text = query.describe().unwrap();
    assert!(!text.contains(r#""from""#));
    assert!(!text.contains(r#""to""#));
    assert!(text.contains(r#""keys":[]"#));
    assert_eq!(Query::parse(&text).unwrap(), query);
    let stripped = r#"{"format":1,"kind":"scan","filter":{},"projection":{"columns":["at"]},"batch":1024,"limit":{"candidates":10000000,"bytes":268435456,"rows":1000000,"memory":268435456}}"#;
    assert_eq!(Query::parse(stripped).unwrap(), query);
}

#[test]
fn refusals() {
    let cases = [
        r#"{"format":2,"kind":"scan","filter":{},"projection":{"columns":["at"]},"batch":1,"limit":{"candidates":1,"bytes":1,"rows":1,"memory":1}}"#,
        r#"{"format":1,"kind":"join","filter":{},"projection":{"columns":["at"]},"batch":1,"limit":{"candidates":1,"bytes":1,"rows":1,"memory":1}}"#,
        r#"{"format":1,"kind":"scan","filter":{},"projection":{"columns":["at"]},"batch":1,"limit":{"candidates":1,"bytes":1,"rows":1,"memory":1},"extra":1}"#,
        r#"{"format":1,"kind":"scan","filter":{"from":9,"to":2},"projection":{"columns":["at"]},"batch":1,"limit":{"candidates":1,"bytes":1,"rows":1,"memory":1}}"#,
        r#"{"format":1,"kind":"scan","filter":{},"projection":{"columns":[]},"batch":1,"limit":{"candidates":1,"bytes":1,"rows":1,"memory":1}}"#,
        r#"{"format":1,"kind":"scan","filter":{},"projection":{"columns":["at","at"]},"batch":1,"limit":{"candidates":1,"bytes":1,"rows":1,"memory":1}}"#,
        r#"{"format":1,"kind":"scan","filter":{},"projection":{"columns":["at"]},"batch":0,"limit":{"candidates":1,"bytes":1,"rows":1,"memory":1}}"#,
        r#"{"format":1,"kind":"scan","filter":{},"projection":{"columns":["at"]},"batch":65537,"limit":{"candidates":1,"bytes":1,"rows":1,"memory":1}}"#,
        r#"{"format":1,"kind":"scan","filter":{},"projection":{"columns":["at"],"count":true},"batch":1,"limit":{"candidates":1,"bytes":1,"rows":1,"memory":1}}"#,
        r#"{"format":1,"kind":"scan","filter":{},"projection":{},"batch":1,"limit":{"candidates":1,"bytes":1,"rows":1,"memory":1}}"#,
        r#"{"format":1,"kind":"scan","filter":{"keys":[{"name":"a","value":"1"},{"name":"a","value":"2"}]},"projection":{"columns":["at"]},"batch":1,"limit":{"candidates":1,"bytes":1,"rows":1,"memory":1}}"#,
    ];
    for text in cases {
        assert!(Query::parse(text).is_err(), "accepted invalid scan: {text}");
    }
}

#[test]
fn oversized() {
    let text = format!(
        r#"{{"format":1,"kind":"scan","filter":{{}},"projection":{{"columns":["at"]}},"batch":1,"limit":{{"candidates":1,"bytes":1,"rows":1,"memory":1}},"pad":"{}"}}"#,
        "x".repeat(1024 * 1024)
    );
    assert!(Query::parse(&text).is_err());
}

fn sides() -> (Side, Side) {
    let mut left = Side::new(
        Filter {
            from: Some(0),
            to: Some(100),
            keys: vec![("source".into(), "acme".into())],
        },
        "source",
        &["source", "kind"],
    );
    left.when = vec![
        Condition::Equal("category".into(), Value::Text("news".into())),
        Condition::Missing("note".into()),
    ];
    let right = Side::new(
        Filter {
            keys: vec![("kind".into(), "click".into())],
            ..Filter::default()
        },
        "kind",
        &["kind", "category"],
    );
    (left, right)
}

#[test]
fn paired() {
    let (left, right) = sides();
    let mut query = Join::new(left, right);
    query.mode = Mode::Inner;
    query.limit.build = 7;
    let text = query.describe().unwrap();
    assert!(text.contains(r#""kind":"join""#));
    assert!(text.contains(r#""mode":"inner""#));
    assert!(text.contains(r#""kind":"equal""#));
    assert!(!text.contains("value\":null,\"int\""));
    assert_eq!(Join::parse(&text).unwrap(), query);
}

#[test]
fn tags() {
    for value in [
        Value::Bool(true),
        Value::Int(-3),
        Value::Uint(3),
        Value::Text("x".into()),
        Value::Bytes(vec![0, 1, 2]),
    ] {
        let mut left = Side::new(Filter::default(), "source", &["source"]);
        left.when = vec![Condition::Equal("flag".into(), value.clone())];
        let query = Join::new(left, Side::new(Filter::default(), "kind", &["kind"]));
        let text = query.describe().unwrap();
        assert_eq!(Join::parse(&text).unwrap(), query);
    }
}

#[test]
fn denied() {
    let base = serde_json::json!({
        "format": 1,
        "kind": "join",
        "left": {"filter": {}, "key": "source", "columns": ["source"], "when": []},
        "right": {"filter": {}, "key": "kind", "columns": ["kind"], "when": []},
        "mode": "left",
        "batch": 1024,
        "limit": {"scan": {"candidates": 1, "bytes": 1, "rows": 1, "memory": 1}, "build": 1, "memory": 1, "work": 1, "rows": 1, "bytes": 1, "batch": 1}
    });
    let ok = serde_json::to_string(&base).unwrap();
    assert!(Join::parse(&ok).is_ok());
    let mut cases = Vec::new();
    let mut changed = base.clone();
    changed["kind"] = serde_json::json!("scan");
    cases.push(serde_json::to_string(&changed).unwrap());
    let mut changed = base.clone();
    changed["mode"] = serde_json::json!("outer");
    cases.push(serde_json::to_string(&changed).unwrap());
    let mut changed = base.clone();
    changed["left"]["when"] =
        serde_json::json!([{"kind": "equal", "column": "flag", "value": {"null": true}}]);
    cases.push(serde_json::to_string(&changed).unwrap());
    let mut changed = base.clone();
    changed["left"]["when"] =
        serde_json::json!([{"kind": "equal", "column": "flag", "value": {"int": 1, "text": "x"}}]);
    cases.push(serde_json::to_string(&changed).unwrap());
    let mut changed = base.clone();
    changed["left"]["when"] =
        serde_json::json!([{"kind": "missing", "column": "flag", "value": {"int": 1}}]);
    cases.push(serde_json::to_string(&changed).unwrap());
    let mut changed = base.clone();
    changed["left"]["when"] = serde_json::json!([{"kind": "equal", "column": "flag"}]);
    cases.push(serde_json::to_string(&changed).unwrap());
    let mut changed = base.clone();
    changed["left"]["when"] =
        serde_json::json!([{"kind": "bigger", "column": "flag", "value": {"int": 1}}]);
    cases.push(serde_json::to_string(&changed).unwrap());
    let mut changed = base.clone();
    changed["left"]["columns"] = serde_json::json!([]);
    cases.push(serde_json::to_string(&changed).unwrap());
    let mut changed = base.clone();
    changed["left"]["when"] = serde_json::json!([
        {"kind": "missing", "column": "flag"},
        {"kind": "missing", "column": "flag"}
    ]);
    cases.push(serde_json::to_string(&changed).unwrap());
    let mut changed = base.clone();
    changed["limit"]["batch"] = serde_json::json!(64 * 1024 * 1024 + 1);
    cases.push(serde_json::to_string(&changed).unwrap());
    let mut changed = base.clone();
    changed["left"]["filter"] = serde_json::json!({"from": 5, "to": 5});
    cases.push(serde_json::to_string(&changed).unwrap());
    for text in cases {
        assert!(Join::parse(&text).is_err(), "accepted invalid join: {text}");
    }
    let _ = Limit::default();
    let _: Query = scan();
    let _: Projection = Projection::Count;
}
