use keel::query::{self, Op, Rank, Slice};

#[tokio::test]
async fn parse() {
    let tree = query::parse("from Student").expect("parse");
    assert_eq!(tree.from(), "Student");
    assert_eq!(tree.slice(), Slice::Live);
    assert!(tree.preds().is_empty());
    assert!(tree.links().is_empty());
    assert!(tree.sort().is_none());
    assert!(tree.limit().is_none());
    assert!(tree.after().is_none());
    assert_eq!(query::digest(&tree), "from student slice live");

    let tree = query::parse(r#"from Student where nickname = "ada""#).expect("where");
    assert_eq!(tree.preds().len(), 1);
    assert_eq!(tree.preds()[0].field(), "nickname");
    assert_eq!(tree.preds()[0].op(), Op::Eq);
    assert_eq!(tree.preds()[0].value(), "ada");
    assert_eq!(
        query::digest(&tree),
        r#"from student slice live where nickname = "ada""#
    );

    let tree = query::parse(r#"from Student where nickname != "ada""#).expect("ne");
    assert_eq!(tree.preds()[0].op(), Op::Ne);

    let tree = query::parse(r#"from Student where nickname in ("ada", "bob")"#).expect("in");
    assert_eq!(tree.preds()[0].op(), Op::In);

    let tree = query::parse(r#"from Student where nickname like "da""#).expect("like");
    assert_eq!(tree.preds()[0].op(), Op::Like);
    assert_eq!(tree.preds()[0].value(), "da");
    assert_eq!(
        query::digest(&tree),
        r#"from student slice live where nickname like "da""#
    );

    let tree = query::parse("from Student link classes").expect("link");
    assert_eq!(tree.links(), &["classes".to_string()]);
    assert_eq!(query::digest(&tree), "from student slice live link classes");

    let tree = query::parse(r#"from Student where courses some (grade = "A")"#).expect("some");
    assert_eq!(tree.preds()[0].op(), Op::Some);
    assert_eq!(tree.preds()[0].field(), "courses");
    assert_eq!(tree.preds()[0].nest().expect("nest").field(), "grade");
    assert_eq!(
        query::digest(&tree),
        r#"from student slice live where courses some (grade = "A")"#
    );

    let tree = query::parse(
        r#"from Student where nickname = "ada" link classes order by nickname limit 2 after "1""#,
    )
    .expect("full");
    assert_eq!(tree.links().len(), 1);
    assert_eq!(tree.limit(), Some(2));
    assert_eq!(
        query::digest(&tree),
        r#"from student slice live where nickname = "ada" link classes order by nickname asc limit 2 after "1""#
    );

    let tree = query::parse("from Student order by nickname").expect("order");
    assert_eq!(tree.sort().expect("sort").field(), "nickname");
    assert_eq!(tree.sort().expect("sort").rank(), Rank::Asc);

    let tree =
        query::parse("from Student order by nickname desc limit 2 after \"3\"").expect("page");
    assert_eq!(tree.after(), Some(3));

    assert!(query::parse("from Student link classes link classes").is_err());
    assert!(query::parse("select *").is_err());
    assert!(query::parse("from Student limit 0").is_err());
    assert!(query::parse("from Student limit 1 order by nickname").is_err());
}
