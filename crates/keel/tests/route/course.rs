use crate::web::{Rig, bag};
use axum::http::StatusCode;
use keel::Graph;
use keel::atom::string;
use keel::resource;
use serde_json::{Value, json};

#[resource]
struct Course {
    #[field(string)]
    code: string,
    #[field(string)]
    title: string,
}

#[resource]
struct Student {
    #[field(string)]
    no: string,
    #[field(string)]
    name: string,
    #[relation(Course, many2many, grade = string)]
    courses: Course,
}

struct Deck {
    rig: Rig,
    algo: i64,
    db: i64,
    net: i64,
    ada: i64,
    bob: i64,
}

async fn deck() -> Deck {
    let mut graph = Graph::new();
    graph.plug::<Course>().plug::<Student>();
    let rig = Rig::open(graph).await;
    let algo = rig
        .made("/course", json!({ "code": "CS101", "title": "algo" }))
        .await;
    let db = rig
        .made("/course", json!({ "code": "CS102", "title": "db" }))
        .await;
    let net = rig
        .made("/course", json!({ "code": "CS103", "title": "net" }))
        .await;
    let ada = rig
        .made("/student", json!({ "no": "S01", "name": "ada" }))
        .await;
    let bob = rig
        .made("/student", json!({ "no": "S02", "name": "bob" }))
        .await;
    let deck = Deck {
        rig,
        algo,
        db,
        net,
        ada,
        bob,
    };
    deck.enroll(ada, algo, "A").await;
    deck.enroll(ada, db, "B").await;
    deck.enroll(ada, net, "").await;
    deck.enroll(bob, algo, "").await;
    deck
}

impl Deck {
    async fn enroll(&self, left: i64, right: i64, grade: &str) {
        let path = format!("/student/{left}/courses");
        let sent = json!({ "right": right, "grade": grade });
        let done = self.rig.post(&path, sent).await;
        assert_eq!(done.status, StatusCode::CREATED, "{}", done.body);
    }

    async fn ties(&self, who: i64) -> Vec<Value> {
        let q = format!(r#"from Student where id = "{who}" link courses"#);
        bag(&self.rig.ask(&q).await, "student.courses")
    }
}

fn right(tie: &Value) -> i64 {
    tie["right"].as_i64().expect("right")
}

fn key(tie: &Value) -> i64 {
    tie["id"].as_i64().expect("id")
}

#[tokio::test]
async fn list() {
    let deck = deck().await;
    let all = deck.rig.get("/course").await;
    assert_eq!(all.status, StatusCode::OK);
    assert_eq!(all.rows().len(), 3);
}

#[tokio::test]
async fn pack() {
    let deck = deck().await;
    let q = r#"from Student where no = "S01" link courses order by no"#;
    let pack = deck.rig.ask(q).await;
    let rows = bag(&pack, "student");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["name"], "ada");
    let ties = bag(&pack, "student.courses");
    assert_eq!(ties.len(), 3);
    let rights: Vec<i64> = ties.iter().map(right).collect();
    assert!(rights.contains(&deck.algo));
    assert!(rights.contains(&deck.db));
    let held: Vec<String> = rights.iter().map(i64::to_string).collect();
    let q = format!(
        r#"from Course where id in ("{}") order by code"#,
        held.join(r#"",""#)
    );
    let list = bag(&deck.rig.ask(&q).await, "course");
    assert_eq!(list.len(), 3);
    assert_eq!(list[0]["code"], "CS101");
}

#[tokio::test]
async fn drop() {
    let deck = deck().await;
    let ties = deck.ties(deck.ada).await;
    let gone = ties
        .iter()
        .find(|tie| right(tie) == deck.db)
        .expect("db tie");
    let path = format!("/student/{}/courses/{}", deck.ada, key(gone));
    assert_eq!(deck.rig.end(&path).await.status, StatusCode::NO_CONTENT);
    let left = deck.ties(deck.ada).await;
    assert_eq!(left.len(), 2);
    assert!(!left.iter().any(|tie| right(tie) == deck.db));
}

#[tokio::test]
async fn rename() {
    let deck = deck().await;
    let path = format!("/student/{}", deck.bob);
    let done = deck.rig.patch(&path, json!({ "name": "bobby" })).await;
    assert_eq!(done.status, StatusCode::OK);
    assert_eq!(done.body["name"], "bobby");
    assert_eq!(done.body["no"], "S02");
}

#[tokio::test]
async fn roster() {
    let deck = deck().await;
    let pack = deck.rig.ask("from Student link courses order by no").await;
    let lefts: Vec<i64> = bag(&pack, "student.courses")
        .iter()
        .filter(|tie| right(tie) == deck.algo)
        .map(|tie| tie["left"].as_i64().expect("left"))
        .collect();
    assert_eq!(lefts.len(), 2);
    assert!(lefts.contains(&deck.ada));
    assert!(lefts.contains(&deck.bob));
}

#[tokio::test]
async fn some() {
    let deck = deck().await;
    let graded = deck
        .rig
        .ask(r#"from Student where courses some (grade = "A")"#)
        .await;
    assert_eq!(bag(&graded, "student").len(), 1);
    let coded = deck
        .rig
        .ask(r#"from Student where courses some (code = "CS101")"#)
        .await;
    assert_eq!(bag(&coded, "student").len(), 2);
}

#[tokio::test]
async fn blocked() {
    let deck = deck().await;
    let path = format!("/course/{}", deck.net);
    assert_eq!(deck.rig.end(&path).await.status, StatusCode::CONFLICT);
    let ties = deck.ties(deck.ada).await;
    let held = ties
        .iter()
        .find(|tie| right(tie) == deck.net)
        .expect("net tie");
    let cut = format!("/student/{}/courses/{}", deck.ada, key(held));
    assert_eq!(deck.rig.end(&cut).await.status, StatusCode::NO_CONTENT);
    assert_eq!(deck.rig.end(&path).await.status, StatusCode::NO_CONTENT);
    let live = deck.rig.ask("from Course order by code").await;
    assert_eq!(bag(&live, "course").len(), 2);
}
