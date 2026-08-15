use keel::atom::string;
use keel::resource;

#[resource]
pub struct Flat {
    #[field(string)]
    note: string,
}

#[resource]
pub struct Deep {
    #[field(string)]
    note: string,
    #[relation(Flat, many2one, root)]
    root: Flat,
}

#[resource]
pub struct Deeper {
    #[field(string)]
    note: string,
    #[relation(Deep, many2one, root)]
    root: Deep,
}
