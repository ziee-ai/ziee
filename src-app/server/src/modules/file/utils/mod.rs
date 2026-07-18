// Utilities for file processing with external tools.
//
// The blob-key derivation (`extension_of`) + upload-security validators
// (`magic`, `zipbomb`) moved to the `ziee-file` SDK crate (chunk `ziee-file`);
// re-exported here so `crate::modules::file::utils::{extension_of,magic,zipbomb}`
// paths resolve unchanged. The processing/format helpers below (pandoc/pdfium/
// spreadsheet/export/embedded) are domain and stay app-side.
pub use ziee_file::utils::{extension_of, magic, zipbomb};

pub mod embedded;
pub mod export;
pub mod pandoc;
pub mod pdfium;
pub mod spreadsheet;
