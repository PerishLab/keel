use super::cell::Cell;
use super::{Condition, Limit, Mode, Query, Side, Stats};
use crate::{Error, Kind, Table, Value};
use arrow_array::RecordBatch;
use arrow_schema::{DataType, Schema};
use std::sync::Arc;

pub(super) struct Plan<'a> {
    pub left: Input<'a>,
    pub right: Input<'a>,
    pub schema: (Arc<Schema>, Arc<Schema>),
}

pub(super) struct Input<'a> {
    pub query: crate::Query,
    pub plan: super::super::plan::Plan,
    pub key: usize,
    pub columns: usize,
    checks: Vec<Check<'a>>,
}

struct Check<'a> {
    column: usize,
    value: Option<&'a Value>,
}

impl<'a> Plan<'a> {
    pub fn new(table: &Table, query: &'a Query) -> Result<Self, Error> {
        if query.limit.batch > 64 * 1024 * 1024 {
            return Err(Error::new("join batch charge must not exceed 64 MiB"));
        }
        let left = Input::new(table, &query.left, query)?;
        let right = Input::new(table, &query.right, query)?;
        let schema = table.schema();
        let ltype = schema.field(table.column(&query.left.key)?).data_type();
        let rtype = schema.field(table.column(&query.right.key)?).data_type();
        if ltype != rtype {
            return Err(Error::new("join keys must have identical declared types"));
        }
        let lcolumns = query
            .left
            .columns
            .iter()
            .map(|name| table.column(name))
            .collect::<Result<Vec<_>, _>>()?;
        let rcolumns = query
            .right
            .columns
            .iter()
            .map(|name| table.column(name))
            .collect::<Result<Vec<_>, _>>()?;
        let lhs = schema.project(&lcolumns)?;
        let mut rhs = schema.project(&rcolumns)?;
        if query.mode == Mode::Left {
            rhs = Schema::new(
                rhs.fields()
                    .iter()
                    .map(|field| field.as_ref().clone().with_nullable(true))
                    .collect::<Vec<_>>(),
            );
        }
        Ok(Self {
            left,
            right,
            schema: (Arc::new(lhs), Arc::new(rhs)),
        })
    }
}

impl<'a> Input<'a> {
    fn new(table: &Table, side: &'a Side, query: &Query) -> Result<Self, Error> {
        let mut names: Vec<_> = side.columns.iter().map(String::as_str).collect();
        let mut input = crate::Query::select(side.filter.clone(), &names);
        input.batch = query.batch;
        input.limit = query.limit.scan.clone();
        table.plan(&input)?;
        if !names.contains(&side.key.as_str()) {
            names.push(&side.key);
        }
        for condition in &side.when {
            let name = match condition {
                Condition::Equal(name, _) | Condition::Missing(name) => name,
            };
            if !names.contains(&name.as_str()) {
                names.push(name);
            }
        }
        input.projection =
            crate::Projection::Columns(names.iter().map(|name| (*name).into()).collect());
        let plan = table.plan(&input)?;
        let schema = table.schema();
        let mut checks = Vec::new();
        for condition in &side.when {
            let (name, value) = match condition {
                Condition::Missing(name) => (name, None),
                Condition::Equal(name, value) => (name, Some(value)),
            };
            if let Some(value) = value {
                if matches!(value, Value::Null) {
                    return Err(Error::new(
                        "equal null is unsupported; use Missing explicitly",
                    ));
                }
                let kind = kind(schema.field(table.column(name)?).data_type());
                if !value.fits(kind) {
                    return Err(Error::new(
                        "join condition value differs from the declared column type",
                    ));
                }
            }
            checks.push(Check {
                column: names.iter().position(|found| *found == name).unwrap(),
                value,
            });
        }
        Ok(Self {
            key: names.iter().position(|name| *name == side.key).unwrap(),
            columns: side.columns.len(),
            checks,
            query: input,
            plan,
        })
    }

    pub fn admits(
        &self,
        batch: &RecordBatch,
        row: usize,
        state: (&Limit, &mut Stats),
    ) -> Result<bool, Error> {
        let (limit, stats) = state;
        for check in &self.checks {
            stats.step(limit)?;
            let cell = Cell::get(batch, check.column, row);
            let matches = match check.value {
                Some(value) => cell.equals(value),
                None => matches!(cell, Cell::Null),
            };
            if !matches {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

fn kind(data: &DataType) -> Kind {
    match data {
        DataType::Boolean => Kind::Bool,
        DataType::Int64 => Kind::Int,
        DataType::UInt64 => Kind::Uint,
        DataType::Utf8 => Kind::Text,
        DataType::Binary => Kind::Bytes,
        _ => unreachable!(),
    }
}
