/**
 * Client-side per-file upload size cap. Single source of truth for every upload
 * surface (chat composer button / drag-drop / paste / attach menu, and the
 * project knowledge-files panel).
 *
 * Mirrors the server default `config.server.max_file_upload_mb` (128 MiB). The
 * server is authoritative — this is a UX pre-check that rejects an oversize file
 * before an upload starts. A deployment that raises the server cap
 * (`ZIEE_MAX_FILE_UPLOAD_MB`) keeps this at the default; the server still accepts
 * the larger file (the client cap only ever pre-rejects, never over-accepts).
 */
export const MAX_FILE_UPLOAD_BYTES = 128 * 1024 * 1024

/** Human-readable label for the cap, used in "file too large" messages. */
export const MAX_FILE_UPLOAD_LABEL = '128MB'
