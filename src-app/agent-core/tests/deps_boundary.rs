//! TEST-36 (INV-8) — the `agent-core` crate's DIRECT dependency set is the port
//! boundary: it may depend on `ai-providers` + `ziee-core` + `ziee-identity` (the
//! shared value/error/identity types), and MUST NOT depend on the `ziee` server
//! crate or any other app module. This keeps the loop domain-neutral: the compiler
//! refuses a build that reaches back into the application.

use std::collections::BTreeSet;

/// Parse the crate names out of `Cargo.toml`'s `[dependencies]` section.
fn direct_deps() -> BTreeSet<String> {
    let manifest = include_str!("../Cargo.toml");
    let mut deps = BTreeSet::new();
    let mut in_deps = false;
    for line in manifest.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_deps = t == "[dependencies]";
            continue;
        }
        if in_deps && !t.is_empty() && !t.starts_with('#') {
            if let Some(name) = t.split(['=', ' ', '{']).next() {
                let name = name.trim();
                if !name.is_empty() {
                    deps.insert(name.to_string());
                }
            }
        }
    }
    deps
}

#[test]
fn agent_core_deps_are_the_port_boundary() {
    let deps = direct_deps();

    // The three ziee-domain crates the ports are built on MUST be present.
    for req in ["ai-providers", "ziee-core", "ziee-identity"] {
        assert!(
            deps.contains(req),
            "agent-core must depend on `{req}` (port boundary); deps = {deps:?}"
        );
    }

    // The ONLY `ziee-*` deps allowed are ziee-core + ziee-identity; the `ziee`
    // server crate (and any framework/app crate) is FORBIDDEN — that would let the
    // domain-neutral loop reach back into the application.
    for d in &deps {
        if d.starts_with("ziee") {
            assert!(
                d == "ziee-core" || d == "ziee-identity",
                "agent-core has a forbidden app dependency `{d}` — the loop must stay \
                 domain-neutral (only ziee-core + ziee-identity allowed); deps = {deps:?}"
            );
        }
        assert_ne!(
            d, "ziee",
            "agent-core must NOT depend on the `ziee` server crate (port boundary)"
        );
    }
}
