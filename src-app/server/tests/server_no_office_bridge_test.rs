//! TEST-16 [ITEM-11] + TEST-14 [ITEM-13] — a plain `ziee` server build carries
//! ZERO office_bridge (module + routes), while the `OfficeBridgeConfig`
//! kill-switch stays server-side and inert. Standalone integration binary — it
//! needs no TestServer/DB, so it does not pull the full harness.

/// TEST-16a: the server's module list (from `MODULE_ENTRIES`) has no office_bridge.
#[test]
fn test16_server_module_list_excludes_office_bridge() {
    let names: Vec<&'static str> = ziee::create_modules().iter().map(|m| m.name()).collect();
    assert!(
        !names.iter().any(|n| n.contains("office_bridge")),
        "server MODULE_ENTRIES must not include office_bridge; got {names:?}"
    );
}

/// TEST-16b: the committed web (ui) OpenAPI exposes no `/office-bridge` route.
#[test]
fn test16_web_openapi_has_no_office_bridge_route() {
    let spec = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../ui/openapi/openapi.json"
    ));
    assert!(
        !spec.contains("office-bridge"),
        "web (ui) OpenAPI must expose no /office-bridge route (office_bridge is desktop-only)"
    );
}

/// TEST-14: the `OfficeBridgeConfig` kill-switch section still deserializes within
/// `ziee` (it stays in `Config` as an inert optional section; DEC-4/DEC-13).
#[test]
fn test14_office_bridge_config_still_deserializes() {
    let c: ziee::OfficeBridgeConfig =
        serde_json::from_value(serde_json::json!({ "enabled": false })).unwrap();
    assert!(!c.enabled, "explicit enabled:false must parse");
    assert!(
        ziee::OfficeBridgeConfig::default().enabled,
        "default OfficeBridgeConfig is enabled"
    );
}
