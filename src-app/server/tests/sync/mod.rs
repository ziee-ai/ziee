//! Realtime-sync integration tests. The HTTP subscribe endpoint (auth gate +
//! event-stream handshake); the security-critical fan-out/audience routing is
//! covered deterministically by the in-source unit tests in
//! `modules/sync/{registry,event}.rs`.

mod subscribe_test;
