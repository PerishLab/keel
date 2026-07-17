#[macro_export]
macro_rules! gate {
    ($actor:ident) => {
        #[::keel::resource(veil)]
        pub struct Token {
            #[field(string)]
            name: ::keel::atom::string,
            #[field(string, unique)]
            hash: ::keel::atom::string,
            #[relation($actor, many2one, root)]
            actor: $actor,
        }

        #[::keel::resource(veil)]
        pub struct Session {
            #[field(string, unique)]
            hash: ::keel::atom::string,
            #[relation($actor, many2one, root)]
            actor: $actor,
        }

        pub fn plug(graph: &mut ::keel::Graph) {
            graph.plug::<Token>().plug::<Session>();
        }
    };
}
