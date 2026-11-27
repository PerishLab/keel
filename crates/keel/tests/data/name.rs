use keel::adapt::db::Sqlite;
use keel::atom::string;
use keel::resource;
use keel::{Core, Ends, Graph};

async fn boot(graph: Graph) -> Core<Sqlite> {
    crate::support::boot(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind")
}

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
    let core = boot(graph).await;
    let units = core.plan().units();
    let label = units.values().find(|u| u.name() == "Label").expect("Label");
    assert_eq!(label.key(), "org:label");
    assert_eq!(label.table(), "org_label");
    let org = units.values().find(|u| u.name() == "Org").expect("Org");
    assert_eq!(org.key(), "org");
    assert_eq!(org.table(), "org");

    let acme = core.put("Org", &[("name", "acme")]).await.expect("org");
    let pair = [("tag", "bug"), ("org", &acme.to_string())];
    core.put("Label", &pair).await.expect("label");
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
    let core = boot(graph).await;

    let units = core.plan().units();
    let keys: Vec<String> = units
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
    let fields = [("word", "sale"), ("shop", &acme.to_string())];
    let tag = core.put("shop:tag", &fields).await.expect("tag");
    let fields = [("body", "hi"), ("team", &red.to_string())];
    let note = core.put("Note", &fields).await.expect("note");
    let ends = Ends {
        left: note,
        right: tag,
    };
    core.tie("Note", "tags", ends, &[]).await.expect("tie");
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
    let core = boot(graph).await;

    let units = core.plan().units();
    let member = units
        .values()
        .find(|u| u.name() == "IssueMember")
        .expect("IssueMember");
    assert_eq!(member.key(), "issueanchor:issuemember");
    assert_eq!(member.table(), "issueanchor_issuemember");

    let claim = units
        .values()
        .find(|u| u.name() == "IssueClaim")
        .expect("IssueClaim");
    assert_eq!(claim.key(), "issuemember:issueclaim");
    assert_eq!(claim.table(), "issuemember_issueclaim");

    let anchor = core
        .put("IssueAnchor", &[("node_id", "n1")])
        .await
        .expect("anchor");
    let link = anchor.to_string();
    let row = [("branch", "main"), ("issue_anchor", link.as_str())];
    let joined = core.put("IssueMember", &row).await.expect("member");
    let link = joined.to_string();
    let row = [
        ("file_path", "src/lib.rs"),
        ("base_version", "1"),
        ("issue_member", link.as_str()),
    ];
    core.put("IssueClaim", &row).await.expect("claim");
    let dup = [
        ("file_path", "src/lib.rs"),
        ("base_version", "2"),
        ("issue_member", link.as_str()),
    ];
    assert!(core.put("IssueClaim", &dup).await.is_err());
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
    assert!(core.has("issueanchor_issuemember").await.expect("member"));
    assert!(core.has("issuemember_issueclaim").await.expect("claim"));
}

#[tokio::test]
async fn marker() {
    use keel::atom::Kind::Text;
    use keel::spec::{Resource, Spec};

    struct Marker;
    #[resource(name = "CustomRoot")]
    struct Custom {
        #[field(string)]
        tag: string,
    }
    #[resource]
    struct Item {
        #[field(string)]
        code: string,
        #[relation(Marker, many2one, root)]
        marker: Marker,
        #[relation(Custom, many2one)]
        custom: Custom,
    }
    assert_eq!(Item::spec().stem(), Some("Marker"));
    let bonds = Item::spec().bonds().to_vec();
    assert!(bonds.iter().any(|b| b.target() == "CustomRoot"));

    let mut graph = Graph::new();
    graph
        .add(Spec::build("Marker").sole("tag", Text).seal())
        .plug::<Custom>()
        .plug::<Item>();
    let core = boot(graph).await;
    let root = core.put("Marker", &[("tag", "m1")]).await.expect("marker");
    let custom = core
        .put("CustomRoot", &[("tag", "c1")])
        .await
        .expect("custom");
    let root = root.to_string();
    let custom = custom.to_string();
    let row = [
        ("code", "i1"),
        ("marker", root.as_str()),
        ("custom", custom.as_str()),
    ];
    core.put("Item", &row).await.expect("item");
    assert!(core.has("marker_item").await.expect("has"));
}
