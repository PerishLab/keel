pub fn key(name: &str, root: Option<&str>) -> String {
    match root {
        Some(root) => format!(
            "{}:{}",
            root.to_ascii_lowercase(),
            name.to_ascii_lowercase()
        ),
        None => name.to_ascii_lowercase(),
    }
}

pub const DESK: &str = "-";

pub(crate) fn kept(name: &str) -> Result<(), crate::adapt::Error> {
    if name == DESK {
        return Err(crate::adapt::Error::Adapt(format!(
            "unit name {DESK} is Keel's"
        )));
    }
    Ok(())
}
