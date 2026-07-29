use super::*;
use crate::adapt::Error;
use crate::life::Work;
use crate::plan::Plan;
use crate::wire::Wire;

pub async fn genesis<W: Wire>(plan: &Plan, wire: &mut W, token: &str) -> Result<(), Error> {
    let node = plan.find(SEAL)?;
    let mut work = Work::new(wire, plan);
    if !work.scan(node).await?.is_empty() {
        return Err(Error::Estate(crate::estate::Fault::Occupied));
    }
    work.put(SEAL, &[("hash", &digest(token))]).await?;
    Ok(())
}

pub async fn sealed<W: Wire>(plan: &Plan, wire: &mut W, token: &str) -> Result<bool, Error> {
    let node = plan.find(SEAL)?;
    let rows = Work::new(wire, plan).scan(node).await?;
    let want = digest(token);
    Ok(rows.first().is_some_and(|row| cell(row, "hash") == want))
}

pub(crate) fn wild() -> Result<String, Error> {
    let mut seed = [0u8; 32];
    getrandom::fill(&mut seed).map_err(|_| Error::Adapt("os entropy unavailable".into()))?;
    Ok(seed.iter().map(|byte| format!("{byte:02x}")).collect())
}

pub(crate) fn token(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(crate) fn digest(token: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}
