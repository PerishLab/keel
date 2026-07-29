use keel::Bootstrap;
use keel::wire::Wire;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

pub async fn sudo<W: Wire>(boot: &mut Bootstrap<W>, path: &Path) -> Result<String, String> {
    match std::fs::read_to_string(path) {
        Ok(token) => Ok(token.trim().to_string()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            let token = boot.mint().await.map_err(|err| err.to_string())?;
            keep(path, &token)?;
            Ok(token)
        }
        Err(err) => Err(err.to_string()),
    }
}

fn keep(path: &Path, token: &str) -> Result<(), String> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|err| err.to_string())?;
    file.write_all(token.as_bytes())
        .map_err(|err| err.to_string())?;
    file.sync_all().map_err(|err| err.to_string())
}
