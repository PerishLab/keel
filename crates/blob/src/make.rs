#[macro_export]
macro_rules! blob {
    ($actor:ident) => {
        #[::keel::resource]
        pub struct Asset {
            #[field(string)]
            name: ::keel::atom::string,
            #[field(string)]
            mime: ::keel::atom::string,
            #[field(int)]
            size: ::keel::atom::int,
            #[field(string, unique)]
            hash: ::keel::atom::string,
            #[relation($actor, many2one, root)]
            owner: $actor,
        }

        pub fn stock(graph: &mut ::keel::Graph) {
            graph.plug::<Asset>();
        }
    };
}
