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

impl Kind {
    pub(crate) fn fit(self, value: &str) -> Result<(), crate::adapt::Error> {
        match self {
            Kind::Text | Kind::Link => Ok(()),
            Kind::Int => value
                .parse::<i64>()
                .map(|_| ())
                .map_err(|_| crate::adapt::Error::Adapt("id needs integer".into())),
            Kind::Bool => match value {
                "true" | "false" => Ok(()),
                _ => Err(crate::adapt::Error::Adapt("value needs bool".into())),
            },
        }
    }
}
