//! TEST-5 [ITEM-4, ITEM-14] — office_bridge's runtime-seam registration is picked
//! up by the chat ExtensionRegistry SNAPSHOT.
//!
//! Regression guard for the phase-6 HIGH finding: the chat module snapshots the
//! registry once at boot (`auto_register_extensions`, inside
//! `start_server_with_routes`), merging the link-time `CHAT_EXTENSIONS` slice with
//! the runtime-registered extensions (`runtime_chat_extensions()`). If
//! office_bridge's `register_chat_extension` push happened AFTER that snapshot
//! (the bug), the office extension would be absent from the merged set → the
//! `attach_office_bridge_mcp` flag would never fire → the whole chat integration
//! would silently no-op. Production now guarantees the ordering by calling
//! `register_office_bridge_static` BEFORE `start_server_with_routes`
//! (`backend/mod.rs`); this test proves the *inclusion* property host-independently
//! by replicating the exact merge `auto_register_extensions` performs.

use ziee_desktop::modules::office_bridge;

#[test]
fn test5_runtime_registered_office_extension_is_in_the_snapshot_merge() {
    // Register the chat extension via the SAME runtime seam
    // `register_office_bridge_static` uses (probe-independent: we're testing the
    // registry merge, not the host gate).
    ziee::chat_extension::register_chat_extension(
        office_bridge::chat_extension::extension::extension_entry(),
    );

    // Replicate auto_register_extensions' merge EXACTLY: link-time slice +
    // runtime-registered, sorted by order. A runtime-registered extension MUST
    // appear here — this is precisely what the ordering bug violated.
    let mut merged: Vec<ziee::chat_extension::ExtensionEntry> =
        ziee::chat_extension::CHAT_EXTENSIONS.iter().copied().collect();
    merged.extend(ziee::chat_extension::runtime_chat_extensions());
    merged.sort_by_key(|e| e.order);

    let office = merged.iter().find(|e| e.name.contains("office"));
    assert!(
        office.is_some(),
        "office_bridge chat extension must be in the snapshot merge \
         (link-time slice + runtime-registered); got {:?}",
        merged.iter().map(|e| e.name).collect::<Vec<_>>()
    );
    // Order 23 — before the MCP tool-collector (order 30) that reads the flag.
    assert_eq!(
        office.unwrap().order,
        23,
        "office extension must keep order 23 so it runs before the MCP collector"
    );

    // Auto-attach side: the entry carries the module's flag + a stable server id,
    // and registering it is an idempotent runtime handoff (no panic).
    let entry = ziee::AutoAttachEntry {
        flag: office_bridge::chat_extension::ATTACH_FLAG,
        server_id: office_bridge::office_bridge_server_id,
    };
    assert_eq!(entry.flag, "attach_office_bridge_mcp");
    assert_eq!((entry.server_id)(), office_bridge::office_bridge_server_id());
    ziee::register_auto_attach_builtin(entry);
}
