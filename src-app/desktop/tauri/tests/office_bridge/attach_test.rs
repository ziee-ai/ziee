//! TEST-5 [ITEM-4, ITEM-14] — office_bridge's runtime-seam registration works
//! cross-crate: registering its chat extension through
//! `ziee::chat_extension::register_chat_extension` surfaces it in the runtime
//! registry, and its auto-attach entry carries the right flag + stable server id.
//! (This is the mechanism `register_office_bridge` uses at desktop boot; here we
//! exercise it in-process from the desktop crate to prove the seam.)

use ziee_desktop::modules::office_bridge;

#[test]
fn test5_office_bridge_runtime_registration_delivers_chat_extension_and_auto_attach() {
    // Chat-extension side: register as register_office_bridge does, then read back.
    ziee::chat_extension::register_chat_extension(
        office_bridge::chat_extension::extension::extension_entry(),
    );
    let names: Vec<&'static str> = ziee::chat_extension::runtime_chat_extensions()
        .iter()
        .map(|e| e.name)
        .collect();
    assert!(
        names.iter().any(|n| n.contains("office")),
        "office_bridge chat extension must be runtime-registered; got {names:?}"
    );

    // Auto-attach side: the entry carries the module's flag + a stable server id.
    let entry = ziee::AutoAttachEntry {
        flag: office_bridge::chat_extension::ATTACH_FLAG,
        server_id: office_bridge::office_bridge_server_id,
    };
    assert_eq!(entry.flag, "attach_office_bridge_mcp");
    assert_eq!((entry.server_id)(), office_bridge::office_bridge_server_id());
    // Registering must not panic (idempotent runtime handoff).
    ziee::register_auto_attach_builtin(entry);
}
