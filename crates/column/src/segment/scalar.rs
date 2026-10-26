use super::frame::{Cursor, narrow, push};
use crate::{Error, Value};

pub(super) fn encode(values: &[Value], buffer: &mut Vec<u8>) -> Result<(), Error> {
    push(buffer, narrow(values.len())?);
    for value in values {
        match value {
            Value::Null => buffer.push(0),
            Value::Bool(value) => buffer.extend([1, u8::from(*value)]),
            Value::Int(value) => {
                buffer.push(2);
                buffer.extend(value.to_le_bytes());
            }
            Value::Uint(value) => {
                buffer.push(3);
                buffer.extend(value.to_le_bytes());
            }
            Value::Text(value) => bytes(buffer, 4, value.as_bytes())?,
            Value::Bytes(value) => bytes(buffer, 5, value)?,
        }
    }
    Ok(())
}

fn bytes(buffer: &mut Vec<u8>, tag: u8, bytes: &[u8]) -> Result<(), Error> {
    buffer.push(tag);
    push(buffer, narrow(bytes.len())?);
    buffer.extend_from_slice(bytes);
    Ok(())
}

impl Cursor<'_> {
    pub(super) fn values(&mut self) -> Option<Vec<Value>> {
        let count = self.word()?;
        (0..count).map(|_| self.value()).collect()
    }

    fn value(&mut self) -> Option<Value> {
        let tag = self.take(1)?[0];
        match tag {
            0 => Some(Value::Null),
            1 => match self.take(1)?[0] {
                0 => Some(Value::Bool(false)),
                1 => Some(Value::Bool(true)),
                _ => None,
            },
            2 => Some(Value::Int(i64::from_le_bytes(
                self.take(8)?.try_into().ok()?,
            ))),
            3 => Some(Value::Uint(u64::from_le_bytes(
                self.take(8)?.try_into().ok()?,
            ))),
            4 | 5 => {
                let length = self.word()? as usize;
                let bytes = self.take(length)?.to_vec();
                if tag == 4 {
                    String::from_utf8(bytes).ok().map(Value::Text)
                } else {
                    Some(Value::Bytes(bytes))
                }
            }
            _ => None,
        }
    }
}
