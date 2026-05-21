/// Resolve a path relative to the repo root, walking up from CWD
/// until we find a marker file. Falls back to CWD-relative.
pub fn repo_relative(suffix: &str) -> std::path::PathBuf {
    let mut cur = std::env::current_dir().unwrap_or_default();
    for _ in 0..6 {
        if cur.join("src-app").is_dir() {
            return cur.join(suffix);
        }
        if !cur.pop() {
            break;
        }
    }
    std::path::PathBuf::from(suffix)
}

/// Find the most-recently-modified `.squashfs` in `dir`, or `None` if
/// the directory has no squashfs files.
pub fn latest_squashfs(dir: &std::path::Path) -> Option<std::path::PathBuf> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("squashfs"))
        .collect();
    entries.sort_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
    entries.last().cloned()
}
