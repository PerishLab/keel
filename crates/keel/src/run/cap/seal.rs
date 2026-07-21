use super::*;
use crate::adapt::Error;
use crate::life::Work;
use crate::plan::Plan;
use crate::wire::Wire;

pub async fn genesis<W: Wire>(plan: &Plan, wire: &mut W) -> Result<Option<String>, Error> {
    let node = plan.find(SEAL)?;
    let mut work = Work::new(wire, plan);
    if !work.scan(node).await?.is_empty() {
        return Ok(None);
    }
    let token = wild();
    work.put(SEAL, &[("hash", &digest(&token))]).await?;
    Ok(Some(token))
}

pub async fn sealed<W: Wire>(plan: &Plan, wire: &mut W, token: &str) -> Result<bool, Error> {
    let node = plan.find(SEAL)?;
    let rows = Work::new(wire, plan).scan(node).await?;
    let want = digest(token);
    Ok(rows.first().is_some_and(|row| cell(row, "hash") == want))
}

pub(crate) fn wild() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    let mut out = String::new();
    for _ in 0..4 {
        let word = RandomState::new().build_hasher().finish();
        out.push_str(&format!("{word:016x}"));
    }
    out
}

pub(crate) fn digest(token: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}
