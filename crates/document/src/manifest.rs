use keel::{Graph, atom::Kind, bond, spec::Spec};

pub(crate) const ROOT: &str = "documentstore";
pub(crate) const DOCUMENT: &str = "documentstore:document";
pub(crate) const REVISION: &str = "documentstore:document:revision";

pub fn stock(graph: &mut Graph) {
    graph.add(
        Spec::build("DocumentStore")
            .sole("nonce", Kind::Text)
            .freeze()
            .seal(),
    );
    graph.add(
        Spec::build("Document")
            .root("store", bond::Kind::Many2one, ROOT)
            .field("collection", Kind::Text)
            .per("name", Kind::Text, &["store", "collection"])
            .field("head", Kind::Text)
            .seal(),
    );
    graph.add(
        Spec::build("Revision")
            .root("document", bond::Kind::Many2one, DOCUMENT)
            .sole("base", Kind::Text)
            .field("token", Kind::Text)
            .freeze()
            .seal(),
    );
}
