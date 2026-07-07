// Global file-preview drawer. Mounted at app root by file/module.tsx
// (always-available; useDelayedFalse keeps it in the tree for the exit
// animation when closed). Reads from `Stores.FilePreviewDrawer`.
//
// Uses the app's shared Drawer wrapper (custom mask + resize handle +
// themed chrome) at default size. The wrapper's title slot accepts a
// ReactNode and prepends an IoIosArrowBack close button automatically,
// so we pass [filename + FilePanelHeaderActions] as title — the Download
// (or viewer-specific HeaderActions) sit in the same row as the back
// arrow and filename. FilePanel runs with `hideHeader` to avoid
// duplicating the filename inside the body.

import { Title } from '@/components/ui'
import { Stores } from '@/core/stores'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { FilePanel, FilePanelHeaderActions } from '@/modules/file/components/FilePanel'

export function FilePreviewDrawer() {
  const isOpen = Stores.FilePreviewDrawer.isOpen
  const file = Stores.FilePreviewDrawer.file

  const titleNode = file ? (
    <Title level={5} className="!m-0 truncate w-full" title={file.filename}>
      {file.filename}
    </Title>
  ) : (
    // Truthy placeholder so the wrapper keeps rendering the back
    // arrow during the exit animation when `file` may briefly be null.
    ' '
  )

  return (
    <Drawer
      open={isOpen}
      onClose={() => Stores.FilePreviewDrawer.closePreview()}
      placement="right"
      title={titleNode}
      // All viewer actions (rendered/raw toggle, copy, download, …) live in the
      // footer now — ghost icon buttons, out of the title row.
      footer={
        file ? (
          <div className="flex items-center justify-end gap-2">
            <FilePanelHeaderActions file={file} />
          </div>
        ) : undefined
      }
      styles={{ body: { padding: 0 } }}
      destroyOnHidden={false}
      // Stack above the knowledge drawer (which uses antd's default
      // zIndex of 1000). Without this, the preview would slide in
      // behind the knowledge drawer when launched from inside it,
      // because both DOM-mount at the app root and z-tied at the
      // default.
      zIndex={1050}
      // Skip the wrapper's vertical-only DivScrollY layer — the file
      // viewer manages both axes of scroll itself (code can be both
      // tall AND wide, and the horizontal scrollbar needs to stay
      // anchored at the viewport bottom edge, not the bottom of an
      // unbounded content box).
      noBodyScrollWrap
    >
      {file ? <FilePanel file={file} hideHeader /> : null}
    </Drawer>
  )
}
