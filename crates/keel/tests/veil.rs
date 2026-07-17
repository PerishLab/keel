use keel::adapt::db::Sqlite;
use keel::atom::string;
use keel::resource;
use keel::{Graph, bind};

#[resource]
struct Actor {
    #[field(string, unique)]
    login: string,
}

#[resource(veil)]
struct Pass {
    #[field(string)]
    hash: string,
    #[relation(Actor, many2one, root)]
    actor: Actor,
}

#[tokio::test]
async fn veiled_units_stay_off_the_plan_projection_but_open_to_faces() {
    let mut graph = Graph::new();
    graph.plug::<Actor>().plug::<Pass>();
    let core = bind(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");

    assert!(core.plan().veiled("Pass"), "Pass should be veiled");
    assert!(!core.plan().veiled("Actor"), "Actor is not veiled");

    let sudo = core.sudo();
    let ada = sudo.put("Actor", &[("login", "ada")]).await.expect("actor");
    let pass = sudo
        .put("Pass", &[("hash", "argon"), ("actor", &ada.to_string())])
        .await
        .expect("ceremony write still works on a veiled unit");
    let pack = sudo.query("from Pass").await.expect("face query works");
    assert_eq!(pack.rows().len(), 1);
    assert_eq!(pack.rows()[0].key(), pass);
}
