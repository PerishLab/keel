#![forbid(unsafe_code)]

mod carrier;
mod error;
mod manifest;
mod model;
mod query;
mod store;
mod view;

pub use manifest::stock;
pub use query::{Condition, Limit, Projection, Query, Selection, Stats};
pub use store::{Head, Record, Store};
pub use view::View;

pub use error::{Error, Failure};
pub use model::{Collection, Field, Model, Shape};
