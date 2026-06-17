//! Deterministic, reversible host-path → in-sandbox-path mapping.
//!
//! A mounted host folder appears in the sandbox at `/mnt/<full host path>`,
//! so when the user mentions a host path the model knows exactly where it is
//! (and vice-versa). The mapping is collision-free (the full path is unique)
//! and handles Windows drive letters:
//!
//! - `/Users/me/runs`   → `/mnt/Users/me/runs`
//! - `C:\data\x`        → `/mnt/C/data/x`
//! - `C:/data/x`        → `/mnt/C/data/x`

/// Map an absolute host path to its deterministic `/mnt/...` sandbox path.
pub fn derive_sandbox_path(host_path: &str) -> String {
    // Normalize Windows separators.
    let mut s = host_path.replace('\\', "/");

    // Strip a Windows drive colon: "C:/x" -> "C/x" (keep the drive letter as a
    // path component so the mapping stays reversible).
    let b = s.as_bytes();
    if b.len() >= 2 && b[1] == b':' && b[0].is_ascii_alphabetic() {
        s = format!("{}{}", &s[0..1], &s[2..]);
    }

    let rel = s.trim_start_matches('/');
    format!("/mnt/{rel}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_unix_paths() {
        assert_eq!(derive_sandbox_path("/Users/me/runs"), "/mnt/Users/me/runs");
        assert_eq!(derive_sandbox_path("/data/genomes/"), "/mnt/data/genomes/");
        assert_eq!(derive_sandbox_path("/a/b/c/d"), "/mnt/a/b/c/d");
    }

    #[test]
    fn maps_windows_paths() {
        assert_eq!(derive_sandbox_path(r"C:\data\x"), "/mnt/C/data/x");
        assert_eq!(derive_sandbox_path("C:/data/x"), "/mnt/C/data/x");
        assert_eq!(derive_sandbox_path(r"D:\runs\sample.bam"), "/mnt/D/runs/sample.bam");
    }

    #[test]
    fn is_reversible_for_unix() {
        // Stripping the "/mnt" prefix recovers the original host path.
        let host = "/Users/me/runs/run5.bam";
        let sandbox = derive_sandbox_path(host);
        assert_eq!(sandbox.strip_prefix("/mnt").unwrap(), host);
    }
}
