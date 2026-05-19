import type { File as FileEntity } from '@/api-client/types'
import type { ComponentType, ReactNode } from 'react'

export interface FileViewerSlotProps {
  file: FileEntity
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
  /** Body content — required. Receives the resolved File entity. */
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
