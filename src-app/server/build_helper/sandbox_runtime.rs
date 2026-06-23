//! Build-time assembly of the mac-arm64 sandbox runtime bundle.
//!
//! Mirrors the pandoc/pdfium/uv/bun build_helpers: produces a single
//! artifact under `binaries/aarch64-apple-darwin/sandbox-runtime/`
//! that the server binary embeds via `include_bytes!`. At runtime the
//! server extracts it to the user's cache dir on first sandbox use.
//!
//! The bundle contains everything the macOS libkrun-backed sandbox
//! needs (so a target Mac doesn't need `brew install libkrun`):
//!   bin/ziee-sandbox-vm-launcher          (built from src-app/sandbox-vm-launcher)
//!   lib/libkrun.1.dylib                   (vendored from brew, install-name rewritten)
//!   lib/libkrunfw.5.dylib
//!   lib/libepoxy.0.dylib
//!   lib/libvirglrenderer.1.dylib
//!   lib/libMoltenVK.dylib
//!   share/guest-root/                     (Alpine userland + cross-built agent)
//!   etc/entitlements.plist                (for runtime ad-hoc codesign)
//!
//! No-op on every non-(macos+aarch64) target.

#![allow(dead_code)]

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Pinned brew formula versions. Bump in lockstep with what's actually
/// installed on the build machine — we read these specific versions
/// from `/opt/homebrew/Cellar/<name>/<version>/lib/`.
const DYLIB_PINS: &[(&str, &str, &str)] = &[
    // (formula, version, leafname-of-dylib-to-copy)
    ("libkrun", "1.18.1", "libkrun.1.dylib"),
    ("libkrunfw", "5.3.0", "libkrunfw.5.dylib"),
    ("libepoxy", "1.5.10", "libepoxy.0.dylib"),
    ("virglrenderer", "0.10.4e", "libvirglrenderer.1.dylib"),
    ("molten-vk", "1.4.1", "libMoltenVK.dylib"),
];

const ALPINE_IMAGE: &str = "alpine:3.20";
const RUST_MUSL_IMAGE: &str = "rust:1.90-alpine3.20";

/// Entry point called from `build.rs`. Lays out the bundle under
/// `<binaries>/<target>/sandbox-runtime/bundle.tar.zst`. Returns Ok(())
/// on non-mac-arm64 targets (no-op).
pub fn setup(target: &str, target_dir: &Path, out_dir: &str) -> Result<(), Box<dyn std::error::Error>> {
    if target != "aarch64-apple-darwin" {
        println!("sandbox-runtime: skipping (target = {target}, not aarch64-apple-darwin)");
        // Ensure the placeholder file exists so include_bytes! always resolves.
        write_placeholder(target_dir)?;
        return Ok(());
    }

    // Escape hatch: developer setting `ZIEE_SKIP_SANDBOX_BUNDLE=1` (e.g.
    // for fast iteration on unrelated code) gets a placeholder. The
    // resulting binary's `code_sandbox::embedded` extracts a zero-byte
    // file and fails loudly at first sandbox use — fine for dev, never
    // for release.
    if std::env::var_os("ZIEE_SKIP_SANDBOX_BUNDLE").is_some() {
        println!("sandbox-runtime: ZIEE_SKIP_SANDBOX_BUNDLE set; writing placeholder");
        write_placeholder(target_dir)?;
        return Ok(());
    }

    let bundle_dir = target_dir.join("sandbox-runtime");
    fs::create_dir_all(&bundle_dir)?;
    let bundle_tar = bundle_dir.join("bundle.tar.zst");

    // Rerun-if-changed: source files that influence the bundle. Cargo
    // re-runs build.rs whenever any of these change.
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").map(PathBuf::from)?;
    let workspace = manifest_dir.parent().ok_or("no workspace parent")?;
    // The same list drives BOTH the `rerun-if-changed` triggers AND the
    // staleness check below, so the two can't drift.
    let watched_sources = [
        "sandbox-vm-launcher/src/main.rs",
        "sandbox-vm-launcher/Cargo.toml",
        "sandbox-vm-launcher/build.rs",
        "sandbox-vm-launcher/entitlements.plist",
        "sandbox-guest-agent/src/main.rs",
        "sandbox-guest-agent/Cargo.toml",
        "sandbox-seccomp/src/lib.rs",
        "sandbox-vm-protocol/src/lib.rs",
    ];
    for src in &watched_sources {
        println!("cargo:rerun-if-changed={}", workspace.join(src).display());
    }

    // Cache hit: bundle exists, is non-empty, AND is at least as new as every
    // watched source. The placeholder path creates a 0-byte file (for
    // non-mac-arm64 / ZIEE_SKIP_SANDBOX_BUNDLE); we redo the real bundle if we
    // land here on a mac-arm64 build that previously had a placeholder.
    //
    // The mtime check is load-bearing: `rerun-if-changed` re-runs build.rs when
    // the agent/launcher/protocol source changes, but WITHOUT this staleness
    // check the helper would keep an existing (now-stale) bundle — so a source
    // change (e.g. a merge that updates the guest agent) silently shipped the OLD
    // agent until someone ran `rm bundle.tar.zst` / `cargo clean`. Comparing the
    // bundle mtime to the newest watched source makes the rebuild automatic.
    // (Manual override still works: `rm bundle.tar.zst` or `cargo clean`.)
    let bundle_meta = fs::metadata(&bundle_tar).ok();
    let cached_ok = bundle_meta.as_ref().map(|m| m.len() > 0).unwrap_or(false) && {
        let bundle_mtime = bundle_meta.as_ref().and_then(|m| m.modified().ok());
        let newest_src = watched_sources
            .iter()
            .filter_map(|p| fs::metadata(workspace.join(p)).and_then(|m| m.modified()).ok())
            .max();
        match (bundle_mtime, newest_src) {
            (Some(bundle), Some(src)) => bundle >= src,
            // Can't read an mtime → fall back to the old "exists + non-empty" rule.
            _ => true,
        }
    };
    if cached_ok {
        println!(
            "sandbox-runtime: bundle already at {} ({} bytes, newer than sources); skipping rebuild",
            bundle_tar.display(),
            fs::metadata(&bundle_tar)?.len()
        );
        return Ok(());
    }

    println!("sandbox-runtime: assembling bundle for {target}");
    let stage = PathBuf::from(out_dir).join("sandbox-runtime-stage");
    if stage.exists() {
        fs::remove_dir_all(&stage)?;
    }
    for sub in &["bin", "lib", "share/guest-root", "etc"] {
        fs::create_dir_all(stage.join(sub))?;
    }

    require_cmd("docker", "Docker is required for cross-compiling the guest agent and assembling the guest root. Install Docker Desktop or OrbStack.")?;
    require_cmd("install_name_tool", "install_name_tool is part of the macOS Command Line Tools. Run `xcode-select --install`.")?;

    // 1. Launcher binary (sibling crate, host build).
    let launcher_src = workspace.join("sandbox-vm-launcher/target/release/ziee-sandbox-vm-launcher");
    if !launcher_src.exists() {
        // Build it. Use a separate CARGO_TARGET_DIR to avoid the parent
        // build's lock (we're inside the server's build.rs).
        println!("sandbox-runtime: building sandbox-vm-launcher");
        let sub_target = PathBuf::from(out_dir).join("launcher-target");
        let status = Command::new(env!("CARGO"))
            .args(["build", "--release"])
            .current_dir(workspace.join("sandbox-vm-launcher"))
            .env("CARGO_TARGET_DIR", &sub_target)
            .status()?;
        if !status.success() {
            return Err("sub-build of sandbox-vm-launcher failed".into());
        }
        let built = sub_target.join("release/ziee-sandbox-vm-launcher");
        fs::copy(&built, stage.join("bin/ziee-sandbox-vm-launcher"))?;
    } else {
        fs::copy(&launcher_src, stage.join("bin/ziee-sandbox-vm-launcher"))?;
    }
    make_executable(&stage.join("bin/ziee-sandbox-vm-launcher"))?;

    // 2. Vendored dylibs from brew Cellar (pinned).
    fetch_dylibs(&stage.join("lib"))?;

    // 3. Cross-compile the guest agent (aarch64-unknown-linux-musl static).
    let agent_path = build_guest_agent(workspace, out_dir)?;
    fs::copy(&agent_path, stage.join("share/guest-root/usr/bin/ziee-sandbox-agent"))
        .or_else(|_| {
            fs::create_dir_all(stage.join("share/guest-root/usr/bin"))?;
            fs::copy(&agent_path, stage.join("share/guest-root/usr/bin/ziee-sandbox-agent"))
        })?;
    make_executable(&stage.join("share/guest-root/usr/bin/ziee-sandbox-agent"))?;

    // 4. Assemble the Alpine userland under share/guest-root/.
    assemble_guest_root(&stage.join("share/guest-root"))?;

    // 5. install_name_tool rewrites — every dylib gets its LC_ID_DYLIB
    //    set to @rpath/<leaf>, and every absolute /opt/homebrew or
    //    /usr/local LC_LOAD_DYLIB entry rewritten to @loader_path/<leaf>.
    //    The launcher gets its rpath rewritten to @executable_path/../lib.
    rewrite_dylibs(&stage.join("lib"))?;
    rewrite_launcher(&stage.join("bin/ziee-sandbox-vm-launcher"))?;

    // 6. Embed the entitlements file for runtime codesign.
    fs::copy(
        workspace.join("sandbox-vm-launcher/entitlements.plist"),
        stage.join("etc/entitlements.plist"),
    )?;

    // 7. Pack tar.zst.
    pack_bundle(&stage, &bundle_tar)?;
    let sz = fs::metadata(&bundle_tar)?.len();
    println!("sandbox-runtime: wrote {} ({} bytes / {:.1} MB)", bundle_tar.display(), sz, sz as f64 / 1_048_576.0);

    Ok(())
}

/// Write an empty placeholder bundle so `include_bytes!` always resolves
/// (even on non-mac-arm64 targets where the bundle is unused).
fn write_placeholder(target_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let bundle_dir = target_dir.join("sandbox-runtime");
    fs::create_dir_all(&bundle_dir)?;
    let bundle_tar = bundle_dir.join("bundle.tar.zst");
    if !bundle_tar.exists() {
        fs::write(&bundle_tar, b"")?;
    }
    Ok(())
}

/// Check that a tool is on PATH. Uses `which`-like resolution rather
/// than running the tool with `--version` because some required tools
/// (notably `install_name_tool`) don't accept `--version` and exit
/// non-zero on argless invocation.
fn require_cmd(name: &str, hint: &str) -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::var_os("PATH").ok_or("PATH not set")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Ok(());
        }
    }
    Err(format!("required tool `{name}` not found on PATH. {hint}").into())
}

fn make_executable(p: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(p)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(p, perms)
    }
    #[cfg(not(unix))]
    {
        let _ = p;
        Ok(())
    }
}

/// Copy pinned dylibs from /opt/homebrew/Cellar into <stage>/lib/.
/// Fails loudly if a pinned version isn't installed — clear remediation
/// is in the error message.
fn fetch_dylibs(lib_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let cellar = Path::new("/opt/homebrew/Cellar");
    for (formula, version, leaf) in DYLIB_PINS {
        let src = cellar.join(formula).join(version).join("lib").join(leaf);
        if !src.exists() {
            return Err(format!(
                "missing pinned dylib: {}\n\
                 hint: `brew tap slp/krun && brew install {}` \
                 (must match version {})",
                src.display(),
                formula,
                version
            )
            .into());
        }
        let dst = lib_dir.join(leaf);
        // Resolve symlinks: brew layouts the dylib as a symlink chain
        // (e.g. libkrun.1.dylib -> libkrun.1.18.1.dylib). Copy the
        // real file so the bundle is self-contained.
        let real = fs::canonicalize(&src)?;
        fs::copy(&real, &dst)?;
        // Strip read-only bit so install_name_tool can rewrite it.
        #[cfg(unix)]
        {
            let mut perms = fs::metadata(&dst)?.permissions();
            use std::os::unix::fs::PermissionsExt;
            perms.set_mode(0o644);
            fs::set_permissions(&dst, perms)?;
        }
        println!("sandbox-runtime: vendored {} ({} bytes)", leaf, fs::metadata(&dst)?.len());
    }
    Ok(())
}

/// Cross-compile sandbox-guest-agent to aarch64-unknown-linux-musl
/// (static, with libseccomp statically linked). Runs inside a
/// linux/arm64 rust:alpine container — needs libseccomp-static for
/// the target arch which apt/brew don't provide on Mac.
fn build_guest_agent(workspace: &Path, out_dir: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    println!("sandbox-runtime: cross-compiling guest agent in {RUST_MUSL_IMAGE}");
    let docker_target = PathBuf::from(out_dir).join("guest-agent-target");
    fs::create_dir_all(&docker_target)?;

    let src_app = workspace.canonicalize()?;
    let status = Command::new("docker")
        .args([
            "run", "--rm",
            "--platform", "linux/arm64",
            "-v", &format!("{}:/work", src_app.display()),
            "-v", &format!("{}:/cargo-target", docker_target.display()),
            "-w", "/work/sandbox-guest-agent",
            "-e", "CARGO_TARGET_DIR=/cargo-target",
            "-e", "CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_RUSTFLAGS=-C target-feature=+crt-static",
            "-e", "LIBSECCOMP_LINK_TYPE=static",
            "-e", "LIBSECCOMP_LIB_PATH=/usr/lib",
            RUST_MUSL_IMAGE,
            "sh", "-c",
            "apk add -q libseccomp-dev libseccomp-static musl-dev build-base pkgconf >/dev/null && \
             cargo build --release --target aarch64-unknown-linux-musl 2>&1 | tail -5",
        ])
        .status()?;
    if !status.success() {
        return Err("guest agent cross-compile failed".into());
    }
    let out = docker_target.join("aarch64-unknown-linux-musl/release/ziee-sandbox-agent");
    if !out.exists() {
        return Err(format!("guest agent binary missing at {}", out.display()).into());
    }
    Ok(out)
}

/// Assemble Alpine userland (busybox, musl, libseccomp, bubblewrap,
/// util-linux) into <guest_root>/. Pre-creates mount points and skips
/// /dev character nodes (can't exist on macOS volumes).
fn assemble_guest_root(guest_root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    println!("sandbox-runtime: assembling guest-root via {ALPINE_IMAGE}");
    let guest_root_abs = guest_root.canonicalize().unwrap_or_else(|_| guest_root.to_path_buf());
    let status = Command::new("docker")
        .args([
            "run", "--rm",
            "--platform", "linux/arm64",
            "-v", &format!("{}:/out", guest_root_abs.display()),
            ALPINE_IMAGE,
            "sh", "-c",
            "set -e
             mkdir -p /root-stage/etc/apk
             cp /etc/apk/repositories /root-stage/etc/apk/
             # Two-step: trust keys first, then real packages.
             apk --root /root-stage --initdb --no-cache add --allow-untrusted alpine-keys >/dev/null 2>&1
             apk --root /root-stage --no-cache add \
                 alpine-baselayout busybox musl libseccomp bubblewrap util-linux >/dev/null 2>&1
             # Pre-create mount points (root is RO at runtime via virtio-fs).
             # /host-mounts is the base for host-folder mounts (feature #3): the
             # agent mounts a tmpfs there, then each extra virtio-fs share at
             # /host-mounts/<i>; bwrap binds those to /mnt/<full host path>.
             mkdir -p /root-stage/proc /root-stage/sandbox-rootfs /root-stage/workspace \
                      /root-stage/sys/fs/cgroup /root-stage/dev /root-stage/tmp /root-stage/run \
                      /root-stage/host-mounts
             # Synthetic identity files baked into the guest root (referenced
             # by GUEST_PASSWD / GUEST_GROUP / GUEST_EMPTY in mac_vm.rs;
             # build_bwrap_argv binds these over /etc/passwd + /etc/group
             # to hide the host user table from sandboxed code).
             echo 'sandboxuser:x:1001:1001:Sandbox User:/home/sandboxuser:/bin/bash' > /root-stage/etc/ziee-sandbox-passwd
             echo 'sandboxuser:x:1001:' > /root-stage/etc/ziee-sandbox-group
             : > /root-stage/etc/ziee-sandbox-empty
             # Sync to volume, skipping /dev character nodes (macOS volume can't host them).
             cd /root-stage
             tar -cf - --exclude=./dev/console --exclude=./dev/null --exclude=./dev/zero \
                       --exclude=./dev/urandom --exclude=./dev/random --exclude=./dev/full \
                       --exclude=./dev/tty --exclude=./usr/bin/ziee-sandbox-agent . \
               | tar -xf - -C /out
             echo \"guest-root: $(du -sh /out | cut -f1)\"",
        ])
        .status()?;
    if !status.success() {
        return Err("guest-root assembly failed".into());
    }
    Ok(())
}

fn rewrite_dylibs(lib_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    for entry in fs::read_dir(lib_dir)? {
        let path = entry?.path();
        if path.extension().map_or(false, |e| e == "dylib") {
            let leaf = path.file_name().unwrap().to_string_lossy().into_owned();
            // 1. ID -> @rpath/<leaf>
            run("install_name_tool", &["-id", &format!("@rpath/{leaf}"), &path.to_string_lossy()])?;
            // 2. Each absolute /opt/homebrew or /usr/local LC_LOAD_DYLIB -> @loader_path/<basename>
            for dep in absolute_load_dylibs(&path)? {
                let dep_leaf = Path::new(&dep).file_name().unwrap().to_string_lossy().into_owned();
                run(
                    "install_name_tool",
                    &["-change", &dep, &format!("@loader_path/{dep_leaf}"), &path.to_string_lossy()],
                )?;
            }
        }
    }
    Ok(())
}

fn rewrite_launcher(launcher: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let launcher_s = launcher.to_string_lossy().into_owned();
    // 1. Drop absolute rpaths (e.g. /opt/homebrew/lib that build.rs added).
    for rp in absolute_rpaths(launcher)? {
        run("install_name_tool", &["-delete_rpath", &rp, &launcher_s])?;
    }
    // 2. Add the bundle-relative rpath. install_name_tool will fail if
    //    it's already present — try, ignore failure.
    let _ = Command::new("install_name_tool")
        .args(["-add_rpath", "@executable_path/../lib", &launcher_s])
        .status();
    // 3. Rewrite absolute LC_LOAD_DYLIB -> @rpath/<leaf>.
    for dep in absolute_load_dylibs(launcher)? {
        let leaf = Path::new(&dep).file_name().unwrap().to_string_lossy().into_owned();
        run("install_name_tool", &["-change", &dep, &format!("@rpath/{leaf}"), &launcher_s])?;
    }
    Ok(())
}

fn absolute_load_dylibs(macho: &Path) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let out = Command::new("otool").args(["-L", &macho.to_string_lossy()]).output()?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    Ok(stdout
        .lines()
        .skip(1)
        .filter_map(|l| l.trim().split_whitespace().next())
        .filter(|s| s.starts_with("/opt/homebrew/") || s.starts_with("/usr/local/"))
        .map(|s| s.to_string())
        .collect())
}

fn absolute_rpaths(macho: &Path) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let out = Command::new("otool").args(["-l", &macho.to_string_lossy()]).output()?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Lines look like `         path /opt/homebrew/lib (offset 12)` after `cmd LC_RPATH`.
    let mut rpaths = Vec::new();
    let mut in_rpath = false;
    for line in stdout.lines() {
        let t = line.trim();
        if t.starts_with("cmd LC_RPATH") {
            in_rpath = true;
            continue;
        }
        if in_rpath {
            if let Some(path) = t.strip_prefix("path ") {
                let path = path.split(" (offset").next().unwrap_or("").trim();
                if path.starts_with("/opt/homebrew/") || path.starts_with("/usr/local/") {
                    rpaths.push(path.to_string());
                }
                in_rpath = false;
            }
        }
    }
    Ok(rpaths)
}

fn run(cmd: &str, args: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    let status = Command::new(cmd).args(args).status()?;
    if !status.success() {
        return Err(format!("{cmd} {args:?} failed (exit {:?})", status.code()).into());
    }
    Ok(())
}

fn pack_bundle(stage: &Path, out: &Path) -> Result<(), Box<dyn std::error::Error>> {
    println!("sandbox-runtime: packing bundle.tar.zst (zstd level 19)");
    // Create tar in-memory (it's only ~25-35 MB after compression),
    // then zstd-compress to file.
    let tar_bytes = {
        let mut buf = Vec::new();
        let mut tar = tar::Builder::new(&mut buf);
        tar.follow_symlinks(false);
        tar.append_dir_all(".", stage)?;
        tar.finish()?;
        drop(tar);
        buf
    };
    let mut f = fs::File::create(out)?;
    let compressed = zstd::encode_all(std::io::Cursor::new(&tar_bytes), 19)?;
    f.write_all(&compressed)?;
    Ok(())
}
