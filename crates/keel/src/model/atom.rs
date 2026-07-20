#[allow(non_camel_case_types)]
pub struct string;

#[allow(non_camel_case_types)]
pub struct url;

#[allow(non_camel_case_types)]
pub struct int;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Text,
    Link,
    Int,
    Bool,
}
