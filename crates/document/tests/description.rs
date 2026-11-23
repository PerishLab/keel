mod support;

use keel_document::{Condition, Error, Projection, Query};
use radicula_document::Path;
use support::document;

fn projection(name: &str, path: &str) -> Projection {
    Projection {
        name: name.into(),
        path: Path::parse(path).unwrap(),
    }
}

fn base() -> Query {
    Query {
        collection: "settings".into(),
        ..Query::default()
    }
}

#[test]
fn description() {
    let query = Query {
        conditions: vec![
            Condition::Equal {
                path: Path::parse("/status").unwrap(),
                value: document("\"paid\"").take(),
            },
            Condition::Missing(Path::parse("/customer/note").unwrap()),
        ],
        select: vec![projection("status", "/status")],
        ..base()
    };
    let held = query.describe().unwrap();
    assert_eq!(
        held,
        r#"{"format":1,"collection":"settings","conditions":[{"kind":"equal","path":"/status","value":"paid"},{"kind":"missing","path":"/customer/note"}],"select":[{"name":"status","path":"/status"}],"limit":{"candidates":100000,"rows":10000,"bytes":67108864}}"#
    );
    assert_eq!(Query::parse(&held).unwrap(), query);
}

#[test]
fn escapes() {
    let query = Query {
        conditions: vec![Condition::Missing(Path::parse("/a~1b/~0c").unwrap())],
        select: vec![projection("root", "")],
        ..base()
    };
    let held = query.describe().unwrap();
    assert!(held.contains(r#""path":"/a~1b/~0c""#));
    assert!(held.contains(r#""path":"""#));
    assert_eq!(Query::parse(&held).unwrap(), query);
}

#[test]
fn refusals() {
    let cases = [
        r#"{"format":2,"collection":"settings","conditions":[],"select":[]}"#,
        r#"{"format":1,"collection":"settings","conditions":[],"select":[],"extra":1}"#,
        r#"{"format":1,"collection":"","conditions":[],"select":[]}"#,
        r#"{"format":1,"collection":"settings","conditions":[{"kind":"missing","path":"bad"}],"select":[]}"#,
        r#"{"format":1,"collection":"settings","conditions":[{"kind":"equal","path":"/x"}],"select":[]}"#,
        r#"{"format":1,"collection":"settings","conditions":[{"kind":"missing","path":"/x","value":1}],"select":[]}"#,
        r#"{"format":1,"collection":"settings","conditions":[{"kind":"other","path":"/x"}],"select":[]}"#,
        r#"{"format":1,"collection":"settings","conditions":[{"kind":"missing","path":"/x"},{"kind":"missing","path":"/x"}],"select":[]}"#,
        r#"{"format":1,"collection":"settings","conditions":[],"select":[{"name":"x","path":"/"},{"name":"x","path":"/y"}]}"#,
        r#"{"format":1,"collection":"settings","conditions":[],"select":[],"limit":{"candidates":"many","rows":1,"bytes":1}}"#,
    ];
    for held in cases {
        assert!(
            Query::parse(held).is_err(),
            "accepted invalid description: {held}"
        );
    }
    assert!(
        Query::parse(r#"{"format":1,"collection":"settings","conditions":[],"select":[]}"#).is_ok()
    );
}

#[test]
fn oversized() {
    let mut conditions = Vec::new();
    for index in 0..257 {
        conditions.push(Condition::Missing(
            Path::parse(&format!("/field{index}")).unwrap(),
        ));
    }
    let query = Query {
        conditions,
        ..base()
    };
    assert!(matches!(query.describe(), Err(Error::Query(_))));
    let held = format!(
        r#"{{"format":1,"collection":"settings","conditions":[],"select":[],"pad":"{}"}}"#,
        "x".repeat(1024 * 1024)
    );
    assert!(matches!(Query::parse(&held), Err(Error::Query(_))));
}
