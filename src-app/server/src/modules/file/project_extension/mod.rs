// File module's contribution to the project-extension system.
//
// This subfolder owns everything related to the project↔file relationship:
// routes mounted at `/api/projects/{id}/files*`, the repository methods
// that read/write the `project_files` join table, the request/response
// types for those routes, and the events emitted on attach/detach.
//
// Acid-test invariant: the project module imports NOTHING from here.
// Discovery is via the `PROJECT_EXTENSIONS` distributed slice (see
// `extension.rs`'s `#[distributed_slice]` static). Deleting this folder
// leaves the project module compiling and running — the slice simply
// loses one entry.

pub mod events;
pub mod extension;
pub mod framing;
pub mod handlers;
pub mod models;
pub mod repository;
pub mod routes;

pub use repository::{PROJECT_MAX_FILES, ProjectFilesRepository};
