use keel::Graph;
use keel::atom::Kind;
use keel::bond;
use keel::spec::{Resource, Spec};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(1);

pub struct Alpha;

impl Resource for Alpha {
    fn name() -> &'static str {
        "Alpha"
    }

    fn spec() -> Spec {
        Spec::build(Self::name())
            .field("zeta", Kind::Text)
            .field("alpha", Kind::Int)
            .seal()
    }
}

pub struct Swap;

impl Resource for Swap {
    fn name() -> &'static str {
        "Alpha"
    }

    fn spec() -> Spec {
        Spec::build(Self::name())
            .field("alpha", Kind::Int)
            .field("zeta", Kind::Text)
            .seal()
    }
}

pub struct Grow;

impl Resource for Grow {
    fn name() -> &'static str {
        "Alpha"
    }

    fn spec() -> Spec {
        Spec::build(Self::name())
            .field("alpha", Kind::Int)
            .field("zeta", Kind::Text)
            .field("beta", Kind::Bool)
            .seal()
    }
}

pub struct Shrink;

impl Resource for Shrink {
    fn name() -> &'static str {
        "Alpha"
    }

    fn spec() -> Spec {
        Spec::build(Self::name()).field("zeta", Kind::Text).seal()
    }
}

pub struct Dup;

impl Resource for Dup {
    fn name() -> &'static str {
        "Dup"
    }

    fn spec() -> Spec {
        Spec::build(Self::name())
            .field("same", Kind::Text)
            .field("same", Kind::Int)
            .seal()
    }
}

pub struct Linker;

impl Resource for Linker {
    fn name() -> &'static str {
        "Cast"
    }

    fn spec() -> Spec {
        Spec::build(Self::name()).field("value", Kind::Link).seal()
    }
}

pub struct Integer;

impl Resource for Integer {
    fn name() -> &'static str {
        "Cast"
    }

    fn spec() -> Spec {
        Spec::build(Self::name()).field("value", Kind::Int).seal()
    }
}

pub struct Words;

impl Resource for Words {
    fn name() -> &'static str {
        "Cast"
    }

    fn spec() -> Spec {
        Spec::build(Self::name()).field("value", Kind::Text).seal()
    }
}

pub struct Club;

impl Resource for Club {
    fn name() -> &'static str {
        "Club"
    }

    fn spec() -> Spec {
        Spec::build(Self::name()).field("name", Kind::Text).seal()
    }
}

pub struct Member;

impl Resource for Member {
    fn name() -> &'static str {
        "Member"
    }

    fn spec() -> Spec {
        Spec::build(Self::name())
            .field("name", Kind::Text)
            .field("age", Kind::Int)
            .bond(
                "clubs",
                bond::Kind::Many2many,
                "Club",
                &[("role", Kind::Text)],
            )
            .seal()
    }
}

pub struct Lean;

impl Resource for Lean {
    fn name() -> &'static str {
        "Member"
    }

    fn spec() -> Spec {
        Spec::build(Self::name())
            .field("name", Kind::Text)
            .bond(
                "clubs",
                bond::Kind::Many2many,
                "Club",
                &[("role", Kind::Text)],
            )
            .seal()
    }
}

pub struct Org;

impl Resource for Org {
    fn name() -> &'static str {
        "Org"
    }

    fn spec() -> Spec {
        Spec::build(Self::name()).field("name", Kind::Text).seal()
    }
}

pub struct Optional;

impl Resource for Optional {
    fn name() -> &'static str {
        "Doc"
    }

    fn spec() -> Spec {
        Spec::build(Self::name())
            .field("title", Kind::Text)
            .free("owner", bond::Kind::Many2one, "Org")
            .seal()
    }
}

pub struct Required;

impl Resource for Required {
    fn name() -> &'static str {
        "Doc"
    }

    fn spec() -> Spec {
        Spec::build(Self::name())
            .field("title", Kind::Text)
            .bond("owner", bond::Kind::Many2one, "Org", &[])
            .seal()
    }
}

pub struct Loose;

impl Resource for Loose {
    fn name() -> &'static str {
        "Label"
    }

    fn spec() -> Spec {
        Spec::build(Self::name()).field("code", Kind::Text).seal()
    }
}

pub struct Sole;

impl Resource for Sole {
    fn name() -> &'static str {
        "Label"
    }

    fn spec() -> Spec {
        Spec::build(Self::name()).sole("code", Kind::Text).seal()
    }
}

pub struct Beta;

impl Resource for Beta {
    fn name() -> &'static str {
        "Beta"
    }

    fn spec() -> Spec {
        Spec::build(Self::name()).field("name", Kind::Text).seal()
    }
}

pub struct Ledger;

impl Resource for Ledger {
    fn name() -> &'static str {
        "Ledger"
    }

    fn spec() -> Spec {
        Spec::build(Self::name()).field("name", Kind::Text).seal()
    }
}

pub struct Ticket;

impl Resource for Ticket {
    fn name() -> &'static str {
        "Ticket"
    }

    fn spec() -> Spec {
        Spec::build(Self::name())
            .field("title", Kind::Text)
            .free("repo", bond::Kind::Many2one, "Ledger")
            .serial("index", "repo")
            .seal()
    }
}

pub fn graph<R: Resource>() -> Graph {
    let mut graph = Graph::new();
    graph.plug::<R>();
    graph
}

pub fn pair<A: Resource, B: Resource>() -> Graph {
    let mut graph = Graph::new();
    graph.plug::<A>().plug::<B>();
    graph
}

pub fn spot(name: &str) -> PathBuf {
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "keel-estate-{}-{name}-{id}.sqlite",
        std::process::id()
    ))
}

pub fn clean(path: &PathBuf) {
    let _ = std::fs::remove_file(path);
}
