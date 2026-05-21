use clap::ArgMatches;

use super::util::repo_relative;

pub fn run(matches: &ArgMatches) -> i32 {
    let keep = *matches.get_one::<usize>("keep").unwrap_or(&2);

    let cache = repo_relative(".ziee-cache/sandbox-rootfs");
    let mut sqfs: Vec<std::path::PathBuf> = match std::fs::read_dir(&cache) {
        Ok(rd) => rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("squashfs"))
            .collect(),
        Err(_) => return 0,
    };
    sqfs.sort_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
    sqfs.reverse(); // newest first
    let to_delete: Vec<_> = sqfs.into_iter().skip(keep).collect();
    for p in &to_delete {
        // Also try to unmount the mountpoint that mirrors the squashfs
        // basename.
        if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
            let mnt = cache.join(stem);
            let _ = std::process::Command::new("fusermount")
                .args(["-u", mnt.to_str().unwrap_or("")])
                .status();
            let _ = std::fs::remove_dir(&mnt);
        }
        if let Err(e) = std::fs::remove_file(p) {
            eprintln!("rm {}: {e}", p.display());
        } else {
            eprintln!("removed {}", p.display());
        }
    }
    0
}
