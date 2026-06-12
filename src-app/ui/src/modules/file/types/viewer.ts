import type { File as FileEntity } from '@/api-client/types'
import type { ComponentType, ReactNode } from 'react'

/**
 * Slot props passed to a viewer's `body` / `headerActions`.
 *
 * Discriminated by which shape is present:
 *
 *  - `{ file: FileEntity }` — the existing right-panel context. The
 *    viewer can use `file.id` to fetch from the FileStore caches
 *    (`thumbnailUrls`, `fileTextContents`, `messageFilesCache`).
 *
 *  - `{ source: { url, name, mimeType?, size? } }` — the new
 *    inline-in-chat context. The file is a tool-result `resource_link`
 *    that has no FileEntity / UUID. The viewer fetches directly from
 *    `source.url` (using `useResourceLinkContent` for text-based bodies).
 *
 * Viewers that only ever render in the right panel can keep handling
 * `{file}` only. Viewers that opt in to `inline` rendering (see
 * `FileViewerEntry.inline`) must handle both shapes — the small
 * `getSource(props)` helper in `file-viewers/shared/source.ts`
 * normalises them.
 */
export type FileViewerSlotProps =
  | { file: FileEntity }
  | { source: InlineFileSource }

/**
 * URL-addressable file passed by the chat-inline render context.
 * Mirrors the fields a tool-result `resource_link` carries.
 */
export interface InlineFileSource {
  /** Download URL — relative `/api/...` for backend-owned artifacts or
   *  an absolute URL for external MCP servers. The chat dispatcher
   *  rejects non-`/api/` URLs as a security guard. */
  url: string
  /** Display name (filename, typically). */
  name: string
  /** Best-known MIME type. May be omitted by some MCP servers — the
   *  viewer-registry's `getViewer` falls back to extension matching. */
  mimeType?: string
  /** File size in bytes, if known. */
  size?: number
  /** Backing File id, when the resource_link points at a server-persisted
   *  artifact (backend-owned). When set, the inline preview renders content
   *  via the authenticated `/api/files/{id}/...` path (same as the right-side
   *  panel) and can open that file in the side panel. Absent for external
   *  MCP links with no backing File. */
  fileId?: string
}

/**
 * A registered file viewer.
 *
 * Each viewer owns its panel content entirely — both the body (`body`) and
 * any header chrome (`headerActions`, optional). The shared `FilePanel`
 * wraps these in the universal layout (title bar + scroll area) and is the
 * only place that knows about panel-wide concerns like the file name and
 * close button.
 *
 * Header chrome and body are independent React components. When they need
 * to coordinate (e.g., a rendered ↔ raw toggle), use the FileStore as the
 * shared state surface (`fileViewModes`, `fileTextContents`, etc.) rather
 * than threading props through the panel.
 */
export interface FileViewerEntry {
  /** Body content — required. Receives EITHER `{file}` or `{source}`
   *  (discriminated union). Viewers that opt in to `inline` must
   *  handle both; viewers that don't only ever see `{file}`. */
  body: ComponentType<FileViewerSlotProps>
  /**
   * Optional header chrome rendered to the right of the panel title.
   * Omit entirely for viewers with no custom actions; the panel still
   * renders the close button.
   */
  headerActions?: ComponentType<FileViewerSlotProps>
  /** Human label used in FileCard subtitle (e.g. "Markdown", "PDF"). */
  label: string
  /** Icon for FileCard. Optional — FileCard falls back to FileTextOutlined. */
  icon?: ReactNode

  /**
   * Opt in to inline-in-chat rendering. When set, the chat dispatcher
   * (`MessageFilesView`) will call this viewer's `body` /
   * `headerActions` with the `{source}` variant of `FileViewerSlotProps`
   * whenever a tool-result `resource_link` matches one of this viewer's
   * `supportedTypes`.
   *
   *  - `false` / undefined: not inline-capable (chat renders a
   *    header-only file card with an "Open in new tab" link as fallback).
   *  - `true`: inline-capable for ALL of this viewer's supportedTypes.
   *  - `FileSupportEntry[]`: inline-capable only for these MIME/ext
   *    rules (must be a subset of supportedTypes). Useful when one
   *    viewer handles multiple types and only some should inline
   *    (e.g., tabular: csv/tsv inline yes, xlsx no).
   *
   *  The decision is owned by the viewer module, never by the chat
   *  side — keeping MIME dispatch in one place.
   */
  inline?: boolean | FileSupportEntry[]

  /**
   * When inline, render the body inside a fixed-height box (with internal
   * scroll) instead of letting it size to its content.
   *
   * Required for bodies that measure their own container height and feed it
   * back into their layout (e.g. the tabular viewer's virtual data grid sets
   * the antd table's `scroll.y` from a `ResizeObserver`). Inline, the preview
   * wrapper only imposes a `max-height`, so such a body's `height: 100%`
   * resolves to `auto` and the measurement loops toward zero. A definite
   * height breaks the loop. Intrinsic bodies (image / text / markdown) size to
   * content and leave this unset.
   */
  inlineFill?: boolean
}

/**
 * One declarative entry describing a file type this viewer can handle.
 * Either `ext` (extension without the dot) or `mime` (MIME type) must be set;
 * if both are set, EITHER matching counts as a match. `mime` supports a
 * trailing `/*` wildcard, e.g. `image/*`.
 *
 * `priority` is a specificity score — lower wins on conflict. Use a higher
 * value (e.g. 10) for fallback/wildcard rules so a more specific rule from
 * another viewer can override it.
 */
export interface FileSupportEntry {
  ext?: string
  mime?: string
  priority?: number
}

export interface FileViewerModule {
  /**
   * Declarative list of file types this viewer handles. Resolution scans
   * all viewers' supportedTypes, computes a per-rule specificity score
   * (lower = more specific), and picks the viewer whose best matching rule
   * has the lowest score. See `fileViewerRegistry.ts`.
   */
  supportedTypes: FileSupportEntry[]
  entry: FileViewerEntry
}

// Backwards-compat alias — some existing viewer files still type their
// component props using the old name. Keep for now; can drop once all
// viewers are migrated and we're sure nothing else imports it.
export type FileViewRendererProps = FileViewerSlotProps
