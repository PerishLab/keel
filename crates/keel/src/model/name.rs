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
