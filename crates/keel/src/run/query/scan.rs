use super::*;
use crate::adapt::Error;

pub(crate) struct Scan<'a> {
    text: &'a str,
    at: usize,
}

impl<'a> Scan<'a> {
    pub(crate) fn new(text: &'a str) -> Self {
        Self { text, at: 0 }
    }

    pub(crate) fn done(&self) -> bool {
        self.at >= self.text.len()
    }

    pub(crate) fn rest(&self) -> &'a str {
        &self.text[self.at..]
    }

    pub(crate) fn ws(&mut self) {
        while let Some(c) = self.rest().chars().next() {
            if !c.is_whitespace() {
                break;
            }
            self.at += c.len_utf8();
        }
    }

    pub(crate) fn kw(&mut self, want: &str) -> Result<(), Error> {
        self.ws();
        if self.opt(want) {
            Ok(())
        } else {
            Err(Error::Adapt(format!("expected {want}")))
        }
    }

    pub(crate) fn opt(&mut self, want: &str) -> bool {
        self.ws();
        let rest = self.rest();
        if rest.len() < want.len() {
            return false;
        }
        if !rest[..want.len()].eq_ignore_ascii_case(want) {
            return false;
        }
        let after = rest.get(want.len()..).unwrap_or("");
        let cont = after
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
        if cont {
            return false;
        }
        self.at += want.len();
        true
    }

    pub(crate) fn ident(&mut self) -> Result<String, Error> {
        self.ws();
        let rest = self.rest();
        let mut end = 0;
        for c in rest.chars() {
            let head = end == 0 && c == '@';
            if c.is_ascii_alphanumeric() || c == '_' || head {
                end += c.len_utf8();
            } else {
                break;
            }
        }
        if end == 0 {
            return Err(Error::Adapt("expected name".into()));
        }
        let name = rest[..end].to_string();
        self.at += end;
        Ok(name)
    }

    pub(crate) fn num(&mut self) -> Result<usize, Error> {
        self.ws();
        let rest = self.rest();
        let mut end = 0;
        for c in rest.chars() {
            if c.is_ascii_digit() {
                end += c.len_utf8();
            } else {
                break;
            }
        }
        if end == 0 {
            return Err(Error::Adapt("expected number".into()));
        }
        let n = rest[..end]
            .parse::<usize>()
            .map_err(|_| Error::Adapt("bad number".into()))?;
        self.at += end;
        Ok(n)
    }

    pub(crate) fn op(&mut self) -> Result<Op, Error> {
        self.ws();
        let rest = self.rest();
        let (op, n) = if rest.starts_with("!=") {
            (Op::Ne, 2)
        } else if rest.starts_with("<=") {
            (Op::Le, 2)
        } else if rest.starts_with(">=") {
            (Op::Ge, 2)
        } else if rest.starts_with('<') {
            (Op::Lt, 1)
        } else if rest.starts_with('>') {
            (Op::Gt, 1)
        } else if rest.starts_with('=') {
            (Op::Eq, 1)
        } else {
            return Err(Error::Adapt("expected op".into()));
        };
        self.at += n;
        Ok(op)
    }

    pub(crate) fn ch(&mut self, want: char) -> Result<(), Error> {
        self.ws();
        let mut chars = self.rest().chars();
        match chars.next() {
            Some(c) if c == want => {
                self.at += c.len_utf8();
                Ok(())
            }
            _ => Err(Error::Adapt(format!("expected {want}"))),
        }
    }

    pub(crate) fn comma(&mut self) -> bool {
        self.ws();
        if self.rest().starts_with(',') {
            self.at += 1;
            true
        } else {
            false
        }
    }

    pub(crate) fn quoted(&mut self) -> Result<String, Error> {
        self.ws();
        let rest = self.rest();
        if !rest.starts_with('"') {
            return Err(Error::Adapt("expected string".into()));
        }
        match take(rest) {
            Some((out, used)) => {
                self.at += used;
                Ok(out)
            }
            None => Err(Error::Adapt("unterminated string".into())),
        }
    }
}

pub(crate) fn take(rest: &str) -> Option<(String, usize)> {
    let bytes = rest.as_bytes();
    let mut i = 1;
    let mut out = String::new();
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'"' {
            return Some((out, i + 1));
        }
        if b == b'\\' {
            i += 1;
            if i >= bytes.len() {
                return None;
            }
            out.push(bytes[i] as char);
            i += 1;
            continue;
        }
        out.push(b as char);
        i += 1;
    }
    None
}
