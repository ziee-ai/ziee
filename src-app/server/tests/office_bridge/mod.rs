//! Integration tests for the `office_bridge` module.
//!
//! TEST-7 (`bridge_test`) exercises the standalone HTTPS + WSS bridge listener
//! (ITEM-5) end-to-end over real TLS, trusting the minted bridge cert.
//!
//! TEST-9 (`windows_com_test`, `#[cfg(windows)]` + `#[ignore]`) is the live
//! Windows COM enumeration + act-on-document test (ITEM-7); it is opt-in and
//! requires a real, non-elevated Office document open on this session.

mod bridge_test;
#[cfg(windows)]
mod windows_com_test;
