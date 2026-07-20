use keel::atom::string;
use keel::resource;

#[resource]
pub struct Actor {
    #[field(string, unique)]
    login: string,
}

#[resource]
pub struct Repo {
    #[field(string, unique = owner)]
    name: string,
    #[field(string)]
    visibility: string,
    #[relation(Actor, many2one, root)]
    owner: Actor,
}

#[resource]
pub struct Issue {
    #[field(string)]
    title: string,
    #[relation(Repo, many2one, root)]
    repo: Repo,
    #[relation(Actor, many2one)]
    author: Actor,
}
