use clap::{ArgMatches, Command};
use linkme::distributed_slice;

/// CLI registration entry. Mirrors [`crate::module_api::ModuleEntry`] but
/// for subcommands that run BEFORE server bootstrap — no DB, Config, or
/// async runtime is available when these fire.
///
/// Each module that wants CLI subcommands declares one static entry:
///
/// ```ignore
/// #[distributed_slice(crate::module_api::CLI_ENTRIES)]
/// static SANDBOX_CLI: CliEntry = CliEntry {
///     name: "code_sandbox",
///     subcommands: || vec![ Command::new("…").arg(…), … ],
///     dispatch: dispatch_fn,
/// };
/// ```
///
/// `subcommands` is a fn pointer (not a static slice) so each closure can
/// build its `Command`s with whatever clap builder calls it needs without
/// being forced into a `const`-friendly shape.
#[derive(Copy, Clone)]
pub struct CliEntry {
    pub name: &'static str,
    pub subcommands: fn() -> Vec<Command>,
    /// Return `Some(exit_code)` if this entry owns the matched subcommand;
    /// `None` to let the next entry try. `main.rs` walks `CLI_ENTRIES` in
    /// link-order and exits 2 if no entry claims a parsed subcommand.
    pub dispatch: fn(&ArgMatches) -> Option<i32>,
}

#[distributed_slice]
pub static CLI_ENTRIES: [CliEntry] = [..];
