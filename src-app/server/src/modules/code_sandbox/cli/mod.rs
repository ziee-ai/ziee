//! Sandbox CLI subcommands.
//!
//! Registers operational commands via `CLI_ENTRIES` so `main.rs`
//! doesn't have to know about them: build, gc. Each command is a
//! small wrapper around either a shell tool (`build.sh`,
//! `fusermount`) or in-process IO. None require Config, a DB pool,
//! or the async runtime — they run before any server bootstrap.
//!
//! There is intentionally NO `mount-sandbox-rootfs` or
//! `fetch-sandbox-rootfs` command. The server lazy-mounts the rootfs
//! on first `execute_command` via `runtime_mount::ensure_rootfs_ready`
//! — including auto-fetching the squashfs from GitHub Releases if
//! it's not in the cache. The whole runtime (download, verify,
//! mount, unmount) is server-owned end-to-end. Operator workflow
//! is install host deps → boot.
//!
//! Air-gapped operators bypass the auto-fetch by manually placing a
//! `ziee-sandbox-rootfs-v{schema}.{revision}-{arch}-{flavor}.squashfs`
//! file in the cache directory; the runtime sees it, skips download,
//! and mounts.

use clap::{value_parser, Arg, ArgMatches, Command};
use linkme::distributed_slice;

use crate::module_api::{CliEntry, CLI_ENTRIES};

mod build;
mod gc;
pub(crate) mod util;

#[distributed_slice(CLI_ENTRIES)]
static SANDBOX_CLI: CliEntry = CliEntry {
    name: "code_sandbox",
    subcommands: build_subcommands,
    dispatch,
};

fn build_subcommands() -> Vec<Command> {
    vec![
        Command::new("build-sandbox-rootfs")
            .about(
                "Build a sandbox rootfs squashfs from src-app/sandbox-rootfs/ sources. \
                 Wraps src-app/sandbox-rootfs/build.sh. Only used by maintainers when \
                 cutting a new rootfs release; operators consume releases via the \
                 server's auto-fetch path.",
            )
            .arg(
                Arg::new("flavor")
                    .long("flavor")
                    .help("Rootfs flavor: minimal (~57 MB) or full (~850 MB)")
                    .default_value("full"),
            )
            .arg(
                Arg::new("output")
                    .long("output")
                    .help("Optional output path override"),
            ),
        Command::new("gc-sandbox-rootfs")
            .about("Remove cached rootfs versions, keeping the N most recent.")
            .arg(
                Arg::new("keep")
                    .long("keep")
                    .help("Number of recent versions to keep")
                    .value_parser(value_parser!(usize))
                    .default_value("2"),
            ),
    ]
}

fn dispatch(top: &ArgMatches) -> Option<i32> {
    let (name, sub) = top.subcommand()?;
    match name {
        "build-sandbox-rootfs" => Some(build::run(sub)),
        "gc-sandbox-rootfs" => Some(gc::run(sub)),
        _ => None,
    }
}
