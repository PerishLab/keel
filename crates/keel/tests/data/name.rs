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

#[tokio::test]
async fn explicit() {
    use keel::atom::int;
    use keel::spec::Resource;

    #[resource(name = "IssueAnchor")]
    struct Anchor {
        #[field(string, name = "node_id", unique)]
        node: string,
    }

    #[resource(name = "IssueMember", veil)]
    struct Member {
        #[field(string)]
        branch: string,
        #[relation(Anchor, one2one, root, name = "issue_anchor")]
        anchor: Anchor,
    }

    #[resource(name = "IssueClaim", frozen)]
    struct Claim {
        #[field(string, name = "file_path", unique = member)]
        path: string,
        #[field(int, name = "base_version", min = 1)]
        version: int,
        #[field(serial, scope = member, name = "claim_no")]
        number: int,
        #[relation(Member, many2one, root, name = "issue_member")]
        member: Member,
    }

    assert_eq!(Anchor::name(), "IssueAnchor");
    assert_eq!(Member::name(), "IssueMember");
    assert_eq!(Claim::name(), "IssueClaim");
    assert!(Member::spec().veiled());
    assert!(Claim::spec().frozen());

    let mut graph = Graph::new();
    graph.plug::<Anchor>().plug::<Member>().plug::<Claim>();
    let core = crate::support::boot(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");

    let member = core
        .plan()
        .units()
        .values()
        .find(|u| u.name() == "IssueMember")
        .expect("IssueMember");
    assert_eq!(member.key(), "issueanchor:issuemember");
    assert_eq!(member.table(), "issueanchor_issuemember");

    let claim = core
        .plan()
        .units()
        .values()
        .find(|u| u.name() == "IssueClaim")
        .expect("IssueClaim");
    assert_eq!(claim.key(), "issuemember:issueclaim");
    assert_eq!(claim.table(), "issuemember_issueclaim");

    let anchor = core
        .put("IssueAnchor", &[("node_id", "n1")])
        .await
        .expect("anchor");
    let joined = core
        .put(
            "IssueMember",
            &[("branch", "main"), ("issue_anchor", &anchor.to_string())],
        )
        .await
        .expect("member");
    core.put(
        "IssueClaim",
        &[
            ("file_path", "src/lib.rs"),
            ("base_version", "1"),
            ("issue_member", &joined.to_string()),
        ],
    )
    .await
    .expect("claim");
    assert!(
        core.put(
            "IssueClaim",
            &[
                ("file_path", "src/lib.rs"),
                ("base_version", "2"),
                ("issue_member", &joined.to_string()),
            ],
        )
        .await
        .is_err()
    );
    let pack = core.query("from IssueClaim").await.expect("pack");
    assert_eq!(pack.rows().len(), 1);
    assert_eq!(
        pack.rows()[0]
            .cells()
            .get("claim_no")
            .map(|cell| cell.show()),
        Some("1".to_string())
    );
    assert!(core.has("issueanchor").await.expect("has anchor"));
    assert!(
        core.has("issueanchor_issuemember")
            .await
            .expect("has member")
    );
    assert!(core.has("issuemember_issueclaim").await.expect("has claim"));
}
