use super::Fault;
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Map, Value};

pub(crate) struct Upload {
    pub(crate) size: i64,
    pub(crate) hash: String,
    checksum: String,
}

impl Upload {
    pub(crate) fn read(body: &Map<String, Value>) -> Result<Self, Fault> {
        let size = match body.get("size") {
            Some(Value::Number(size)) => size.as_i64(),
            Some(Value::String(size)) => size.parse().ok(),
            _ => None,
        }
        .filter(|size| *size >= 0)
        .ok_or_else(Fault::bad)?;
        let hash = body
            .get("hash")
            .and_then(Value::as_str)
            .ok_or_else(Fault::bad)?;
        if hash.len() != 64
            || !hash
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
        {
            return Err(Fault::bad());
        }
        let mut bytes = [0; 32];
        for (at, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&hash[at * 2..at * 2 + 2], 16).map_err(|_| Fault::bad())?;
        }
        Ok(Self {
            size,
            hash: hash.into(),
            checksum: STANDARD.encode(bytes),
        })
    }

    pub(crate) fn headers(&self) -> [(&'static str, String); 2] {
        [
            ("content-length", self.size.to_string()),
            ("x-amz-checksum-sha256", self.checksum.clone()),
        ]
    }
}
