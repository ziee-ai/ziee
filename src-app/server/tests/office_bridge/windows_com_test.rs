//! TEST-9 (ITEM-7) — live Windows COM enumeration + act-on-document.
//!
//! `#[cfg(windows)]` + `#[ignore]`: this is a genuine *live* test — it requires
//! a real, **non-elevated** Word/Excel/PowerPoint document open on the same
//! desktop session + integrity as the test process, so it can never run in CI
//! or in the normal suite. It stays `#[ignore]` (opt-in / manual), mirroring the
//! sandbox rootfs tiers.
//!
//! Run it manually, with at least one Office document open, via:
//!
//! ```text
//! cargo test -p ziee --test integration_tests -- --ignored office_bridge::windows_com
//! ```
//!
//! It asserts the spike-proven flow:
//!   1. `list_open_documents` returns ≥1 open doc, at least one of which carries
//!      a full path (a saved document) and a non-empty `attach_method`.
//!   2. `act_on_document(AppendParagraph)` against an open **Word** doc appends,
//!      saves, and reads back the appended text (so the round-trip landed).
//!
//! With no Word document open, step 2 is skipped with an explanatory message
//! (the test still asserts step 1) — so it is safe to run with only Excel/PPT
//! open, but the full assertion needs Word.

#![cfg(windows)]

use ziee::office_bridge_platform::{self as platform, DocOp, OfficeApp};

#[tokio::test]
#[ignore = "live: requires a non-elevated Office document open on this session"]
async fn test9_windows_com_list_and_act() {
    let office = platform::active();

    // The host must actually be a supported desktop with Office; if the probe
    // says otherwise the environment is wrong for this manual test.
    let caps = office
        .probe()
        .expect("Windows host must probe as a supported desktop");
    assert!(caps.desktop, "Windows must report desktop=true");

    // ---- (1) enumerate ----
    let docs = office
        .list_open_documents()
        .await
        .expect("list_open_documents must not error on a supported desktop");
    assert!(
        !docs.is_empty(),
        "TEST-9 requires at least one open Office document; found none. \
         Open a saved Word/Excel/PowerPoint doc (non-elevated) and re-run."
    );

    // Every enumerated doc must carry a diagnostic attach method.
    for d in &docs {
        assert!(
            !d.attach_method.is_empty(),
            "each OpenDoc must record how it was attached"
        );
    }

    // At least one saved doc should expose a full path (the spike-proven fact:
    // COM FullName + Path are populated for saved docs).
    assert!(
        docs.iter().any(|d| d.path.is_some() && !d.full_name.is_empty()),
        "expected at least one saved document with a full path; \
         got: {docs:?}. Save the open document and re-run."
    );

    // ---- (2) act on a Word doc (append + save + read-back) ----
    let Some(word_doc) = docs.iter().find(|d| d.app == OfficeApp::Word) else {
        eprintln!(
            "TEST-9: no Word document open — skipping act_on_document assertion. \
             Open a Word doc to exercise the full append/save/read-back path."
        );
        return;
    };

    let stamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S");
    let marker = format!("ziee office_bridge TEST-9 append {stamp}");
    let res = office
        .act_on_document(
            &word_doc.full_name,
            &DocOp::AppendParagraph {
                text: marker.clone(),
            },
        )
        .await
        .expect("act_on_document(AppendParagraph) on an open Word doc must succeed");

    assert!(res.ok, "act result must be ok");
    let read_back = res
        .read_back
        .expect("act_on_document must read back the last paragraph text");
    assert!(
        read_back.contains(&marker),
        "read-back last paragraph ({read_back:?}) must contain the appended marker ({marker:?})"
    );
}
