use keel::atom::{int, string, url};
use keel::resource;

#[resource]
pub struct Class {
    #[field(string)]
    title: string,
}

#[resource]
pub struct Student {
    #[field(string)]
    nickname: string,
    #[field(url)]
    avatar: url,
    #[relation(Class, many2many)]
    classes: Class,
}

#[resource]
pub struct Score {
    #[field(string)]
    name: string,
    #[field(int)]
    points: int,
    #[field(bool)]
    passed: bool,
}
