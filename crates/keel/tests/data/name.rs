use keel::adapt::db::Sqlite;
use keel::atom::string;
use keel::resource;
use keel::{Ends, Graph};

#[tokio::test]
async fn rooted() {
    #[resource]
    struct Org {
        #[field(string)]
        name: string,
    }

    #[resource]
    struct Label {
        #[field(string)]
        tag: string,
        #[relation(Org, many2one, root)]
        org: Org,
    }

    let mut graph = Graph::new();
    graph.plug::<Org>().plug::<Label>();
    let core = crate::support::boot(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");
    let label = core
        .plan()
        .units()
        .values()
        .find(|u| u.name() == "Label")
        .expect("Label");
    assert_eq!(label.key(), "org:label");
    assert_eq!(label.table(), "org_label");
    let org = core
        .plan()
        .units()
        .values()
        .find(|u| u.name() == "Org")
        .expect("Org");
    assert_eq!(org.key(), "org");
    assert_eq!(org.table(), "org");

    let acme = core.put("Org", &[("name", "acme")]).await.expect("org");
    core.put("Label", &[("tag", "bug"), ("org", &acme.to_string())])
        .await
        .expect("label");
    let pack = core.query("from Label").await.expect("pack");
    assert_eq!(pack.rows().len(), 1);
    assert!(core.has("org_label").await.expect("has"));
    assert!(!core.has("label").await.expect("no bare"));
}

#[tokio::test]
async fn twin() {
    mod shop {
        use keel::atom::string;
        use keel::resource;
        #[resource]
        pub struct Shop {
            #[field(string)]
            pub sign: string,
        }
        #[resource]
        pub struct Tag {
            #[field(string)]
            pub word: string,
            #[relation(Shop, many2one, root)]
            pub shop: Shop,
        }
    }
    mod team {
        use keel::atom::string;
        use keel::resource;
        #[resource]
        pub struct Team {
            #[field(string)]
            pub handle: string,
        }
        #[resource]
        pub struct Tag {
            #[field(string)]
            pub word: string,
            #[relation(Team, many2one, root)]
            pub team: Team,
        }
    }
    #[resource]
    struct Note {
        #[field(string)]
        body: string,
        #[relation(Team, many2one, root)]
        team: team::Team,
        #[relation(Shop::Tag, many2many)]
        tags: shop::Tag,
    }

    let mut graph = Graph::new();
    graph
        .plug::<shop::Shop>()
        .plug::<shop::Tag>()
        .plug::<team::Team>()
        .plug::<team::Tag>()
        .plug::<Note>();
    let core = crate::support::boot(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");

    let keys: Vec<String> = core
        .plan()
        .units()
        .values()
        .filter(|u| u.name() == "Tag")
        .map(|u| u.key())
        .collect();
    assert_eq!(keys.len(), 2);
    assert!(keys.iter().any(|k| k == "shop:tag"));
    assert!(keys.iter().any(|k| k == "team:tag"));

    assert!(core.live("Tag").await.is_err());
    core.live("shop:tag").await.expect("shop tag");
    core.live("team:tag").await.expect("team tag");

    let acme = core.put("Shop", &[("sign", "acme")]).await.expect("shop");
    let red = core.put("Team", &[("handle", "red")]).await.expect("team");
    let tag = core
        .put("shop:tag", &[("word", "sale"), ("shop", &acme.to_string())])
        .await
        .expect("tag");
    let note = core
        .put("Note", &[("body", "hi"), ("team", &red.to_string())])
        .await
        .expect("note");
    core.tie(
        "Note",
        "tags",
        Ends {
            left: note,
            right: tag,
        },
        &[],
    )
    .await
    .expect("tie");
    let pack = core
        .query(r#"from Note where team = "1" link tags"#)
        .await
        .expect("pack");
    assert_eq!(pack.bond("team:note.tags").expect("bag").len(), 1);
    assert!(core.has("shop_tag").await.expect("has shop tag"));
    assert!(core.has("team_tag").await.expect("has team tag"));
    assert!(core.has("team_note_tags").await.expect("has join"));
}
