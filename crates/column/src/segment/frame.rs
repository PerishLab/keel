use crate::Record;

const TOKEN: u8 = 0;
const ENTRY: u8 = 1;
const HEAD: usize = 8;

pub(crate) enum Frame {
    Token(String),
    Entry(Record),
}

pub(crate) fn encode(frame: &Frame) -> Vec<u8> {
    let mut payload = Vec::new();
    match frame {
        Frame::Token(token) => {
            payload.push(TOKEN);
            payload.extend_from_slice(token.as_bytes());
        }
        Frame::Entry(record) => {
            payload.push(ENTRY);
            payload.extend_from_slice(&record.time.to_le_bytes());
            push(&mut payload, narrow(record.keys.len()));
            for key in &record.keys {
                push(&mut payload, narrow(key.len()));
                payload.extend_from_slice(key.as_bytes());
            }
            push(&mut payload, narrow(record.raw.len()));
            payload.extend_from_slice(&record.raw);
        }
    }
    let mut framed = Vec::with_capacity(HEAD + payload.len());
    push(&mut framed, narrow(payload.len()));
    push(&mut framed, crc32fast::hash(&payload));
    framed.extend_from_slice(&payload);
    framed
}

pub(crate) fn decode(bytes: &[u8]) -> Vec<(Frame, usize)> {
    let mut frames = Vec::new();
    let mut cursor = Cursor { bytes, at: 0 };
    while let Some(frame) = cursor.frame() {
        frames.push((frame, cursor.at));
    }
    frames
}

struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Cursor<'_> {
    fn frame(&mut self) -> Option<Frame> {
        let start = self.at;
        let length = self.word()? as usize;
        let checksum = self.word()?;
        let payload = self.take(length)?;
        if crc32fast::hash(payload) != checksum {
            self.at = start;
            return None;
        }
        let parsed = Cursor {
            bytes: payload,
            at: 0,
        }
        .payload();
        if parsed.is_none() {
            self.at = start;
        }
        parsed
    }

    fn payload(mut self) -> Option<Frame> {
        match *self.take(1)?.first()? {
            TOKEN => String::from_utf8(self.bytes[self.at..].to_vec())
                .ok()
                .map(Frame::Token),
            ENTRY => {
                let time = u64::from_le_bytes(self.take(8)?.try_into().ok()?);
                let count = self.word()?;
                let keys = (0..count)
                    .map(|_| {
                        let length = self.word()? as usize;
                        String::from_utf8(self.take(length)?.to_vec()).ok()
                    })
                    .collect::<Option<Vec<_>>>()?;
                let length = self.word()? as usize;
                let raw = self.take(length)?.to_vec();
                Some(Frame::Entry(Record { raw, time, keys }))
            }
            _ => None,
        }
    }

    fn word(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }

    fn take(&mut self, length: usize) -> Option<&[u8]> {
        let end = self.at.checked_add(length)?;
        let slice = self.bytes.get(self.at..end)?;
        self.at = end;
        Some(slice)
    }
}

fn push(buffer: &mut Vec<u8>, value: u32) {
    buffer.extend_from_slice(&value.to_le_bytes());
}

fn narrow(length: usize) -> u32 {
    u32::try_from(length).unwrap_or(u32::MAX)
}
