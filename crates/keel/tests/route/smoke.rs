use crate::web::{Rig, bag, root};
use axum::http::StatusCode;
use keel::Graph;
use keel::atom::string;
use keel::resource;
use serde_json::json;

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

async fn rig() -> Rig {
    let mut graph = Graph::new();
    graph.plug::<Course>().plug::<Student>();
    Rig::open(graph).await
}

#[tokio::test]
async fn rest() {
    let rig = rig().await;
    let student = rig
        .made("/student", json!({ "no": "S01", "name": "ada" }))
        .await;
    let list = rig.get("/student").await;
    assert_eq!(list.status, StatusCode::OK);
    assert!(!list.rows().is_empty());
    let one = rig.get(&format!("/student/{student}")).await;
    assert_eq!(one.status, StatusCode::OK);
    assert_eq!(one.body["name"], "ada");
}

#[tokio::test]
async fn edit() {
    let rig = rig().await;
    let student = rig
        .made("/student", json!({ "no": "S01", "name": "ada" }))
        .await;
    let sent = json!({ "name": "ada2" });
    let done = rig.patch(&format!("/student/{student}"), sent).await;
    assert_eq!(done.status, StatusCode::OK);
    assert_eq!(done.body["name"], "ada2");
    assert_eq!(done.body["no"], "S01");
}

#[tokio::test]
async fn refuse() {
    let rig = rig().await;
    let student = rig
        .made("/student", json!({ "no": "S01", "name": "ada" }))
        .await;
    let read = rig.get(&format!("/student/{student}/courses")).await;
    assert_eq!(read.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn filter() {
    let rig = rig().await;
    rig.made("/student", json!({ "no": "S01", "name": "ada" }))
        .await;
    rig.made("/student", json!({ "no": "S02", "name": "bob" }))
        .await;
    let all = rig.ask("from Student").await;
    assert_eq!(root(&all), "student");
    assert!(!bag(&all, "student").is_empty());
    let one = rig.ask(r#"from Student where no = "S01""#).await;
    let rows = bag(&one, "student");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["name"], "ada");
}

#[tokio::test]
async fn scalar() {
    let rig = rig().await;
    rig.made("/student", json!({ "no": "S01", "name": "ada" }))
        .await;
    rig.made("/student", json!({ "no": "S02", "name": "bob" }))
        .await;
    let q = r#"from Student where name != "ada" and name in ("bob", "zoe")"#;
    let rows = bag(&rig.ask(q).await, "student");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["name"], "bob");
}

#[tokio::test]
async fn page() {
    let rig = rig().await;
    let ada = rig
        .made("/student", json!({ "no": "S01", "name": "ada" }))
        .await;
    let bob = rig
        .made("/student", json!({ "no": "S02", "name": "bob" }))
        .await;
    let head = bag(
        &rig.ask("from Student order by name desc limit 1").await,
        "student",
    );
    assert_eq!(head.len(), 1);
    assert_eq!(head[0]["name"], "bob");
    let q = format!(r#"from Student order by name desc limit 1 after "{bob}""#);
    let next = bag(&rig.ask(&q).await, "student");
    assert_eq!(next.len(), 1);
    assert_eq!(next[0]["name"], "ada");
    let q = format!(r#"from Student where id in ("{ada}", "{bob}") order by id"#);
    assert_eq!(bag(&rig.ask(&q).await, "student").len(), 2);
}

#[tokio::test]
async fn edge() {
    let rig = rig().await;
    let course = rig
        .made("/course", json!({ "code": "CS101", "title": "algo" }))
        .await;
    let student = rig
        .made("/student", json!({ "no": "S01", "name": "ada" }))
        .await;
    let path = format!("/student/{student}/courses");
    let tie = rig.post(&path, json!({ "right": course })).await;
    assert_eq!(tie.status, StatusCode::CREATED);
    let q = format!(r#"from Student where id = "{student}" link courses"#);
    let pack = rig.ask(&q).await;
    let ties = bag(&pack, "student.courses");
    assert_eq!(ties.len(), 1);
    assert_eq!(ties[0]["right"], course);
    assert!(bag(&pack, "course").is_empty());
}

#[tokio::test]
async fn batch() {
    let rig = rig().await;
    let sent = json!({
        "deeds": [
            { "verb": "put", "unit": "Course", "fields": { "code": "M1", "title": "algebra" } },
            { "verb": "put", "unit": "Course", "fields": { "code": "M2", "title": "calculus" } }
        ]
    });
    let done = rig.post("/batch", sent).await;
    assert_eq!(done.status, StatusCode::OK, "{}", done.body);
    assert_eq!(done.body["ids"].as_array().expect("ids").len(), 2);
    let back = rig.ask(r#"from Course where code = "M2""#).await;
    assert_eq!(bag(&back, "course").len(), 1);
}
