import { useEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import {
  App,
  Button,
  Empty,
  Popconfirm,
  Progress,
  Spin,
  Tag,
  Tooltip,
  Typography,
  Upload,
  theme,
} from 'antd'
import type { UploadProps } from 'antd'
import {
  CloseCircleOutlined,
  DeleteOutlined,
  FileOutlined,
  UploadOutlined,
} from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import {
  Permissions,
  type File as ProjectFile,
} from '@/api-client/types'

interface ProjectFilesPanelProps {
  projectId: string
  /** Forwarded to the panel's root `<div>`. The Drawer wrapper injects
   *  `w-full` here via React.cloneElement so the panel fills the drawer's
   *  body width — without this passthrough, the root `<div>` defaults to
   *  content-width and the drawer body looks short. */
  className?: string
}

/**
 * 100-file cap per project (server-enforced via PROJECT_MAX_FILES).
 * The UI mirrors the constant here so users see a counter + warning
 * before they hit a 422. Closes audit F2.
 */
const PROJECT_FILE_CAP = 100

/// 100 MiB — mirrors the chat file extension's pre-flight check, which
/// mirrors the backend's per-file cap. Lets us reject obvious overrun
/// client-side before burning a multipart round-trip.
const MAX_FILE_SIZE = 100 * 1024 * 1024

function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  if (bytes < 1024 * 1024 * 1024)
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`
}

export function ProjectFilesPanel({
  projectId,
  className,
}: ProjectFilesPanelProps) {
  const { message } = App.useApp()
  const { token } = theme.useToken()
  const { files, filesLoading, uploadingFiles } = Stores.ProjectDetail
  const canEdit = usePermission(Permissions.ProjectsEdit)
  const canUpload = canEdit && usePermission(Permissions.FilesUpload)

  const count = files.length
  const atCap = count >= PROJECT_FILE_CAP
  const nearCap = count >= PROJECT_FILE_CAP - 5 && !atCap

  const rootRef = useRef<HTMLDivElement>(null)
  const [drawerBody, setDrawerBody] = useState<HTMLElement | null>(null)
  const [isDragging, setIsDragging] = useState(false)

  const handleDetach = async (file: ProjectFile) => {
    try {
      await Stores.ProjectDetail.detachFile(projectId, file.id)
      message.success('File removed from project')
    } catch (err) {
      message.error(
        err instanceof Error ? err.message : 'Failed to detach file',
      )
    }
  }

  // Pre-flight + dispatch. Stable identity via ref so the DOM event
  // listener attached in useEffect always calls the latest version
  // (capturing `atCap` etc. correctly) without having to re-attach
  // listeners on every render.
  const dispatchFiles = (incoming: File[]) => {
    if (incoming.length === 0) return
    if (atCap) {
      message.error(
        `Project is at the ${PROJECT_FILE_CAP}-file cap. Remove a file first.`,
      )
      return
    }
    const accepted: File[] = []
    for (const file of incoming) {
      if (file.size > MAX_FILE_SIZE) {
        message.error(
          `${file.name} is too large (max ${formatFileSize(MAX_FILE_SIZE)}).`,
        )
        continue
      }
      accepted.push(file)
    }
    if (accepted.length === 0) return
    void Stores.ProjectDetail.uploadAndAttachFiles(projectId, accepted)
  }
  const dispatchRef = useRef(dispatchFiles)
  dispatchRef.current = dispatchFiles

  // Attach drag handlers to the antd Drawer's BODY element (the
  // ancestor with class `ant-drawer-body`). That element is the only
  // one in the chain that's guaranteed to span the full drawer
  // height — DivScrollY's internal viewport doesn't propagate
  // height to its children, so attaching handlers to the panel root
  // only catches drops over the file list, not the empty area
  // below. We also render the visual overlay as a Portal child of
  // the body element so it covers the same area regardless of what
  // DivScrollY / our own column layout produces.
  useEffect(() => {
    if (!canUpload) return
    const body = rootRef.current?.closest(
      '.ant-drawer-body',
    ) as HTMLElement | null
    if (!body) return
    setDrawerBody(body)

    // Make sure absolute children (our overlay) anchor to the body,
    // not the viewport. antd's `.ant-drawer-body` is `position:
    // relative` in current versions, but assert it anyway in case
    // a future antd change drops it.
    const previousPosition = body.style.position
    if (!body.style.position && getComputedStyle(body).position === 'static') {
      body.style.position = 'relative'
    }

    // dragCounter handles the child-element bubble: dragenter on a
    // CHILD bubbles to the wrapper as a fresh dragenter, then leaving
    // that child fires a dragleave even though we're still over the
    // wrapper. Counting enter/leave pairs and only hiding when the
    // counter hits 0 fixes the flicker.
    let dragCounter = 0

    const onDragEnter = (e: DragEvent) => {
      if (!Array.from(e.dataTransfer?.types ?? []).includes('Files')) return
      e.preventDefault()
      dragCounter += 1
      if (dragCounter === 1) setIsDragging(true)
    }
    const onDragLeave = (e: DragEvent) => {
      if (!Array.from(e.dataTransfer?.types ?? []).includes('Files')) return
      e.preventDefault()
      dragCounter = Math.max(0, dragCounter - 1)
      if (dragCounter === 0) setIsDragging(false)
    }
    const onDragOver = (e: DragEvent) => {
      if (!Array.from(e.dataTransfer?.types ?? []).includes('Files')) return
      // Required for drop to fire — tells the browser this element
      // accepts the drop.
      e.preventDefault()
    }
    const onDrop = (e: DragEvent) => {
      if (!Array.from(e.dataTransfer?.types ?? []).includes('Files')) return
      e.preventDefault()
      dragCounter = 0
      setIsDragging(false)
      const dropped = Array.from(e.dataTransfer?.files ?? [])
      dispatchRef.current(dropped)
    }

    body.addEventListener('dragenter', onDragEnter)
    body.addEventListener('dragleave', onDragLeave)
    body.addEventListener('dragover', onDragOver)
    body.addEventListener('drop', onDrop)

    return () => {
      body.removeEventListener('dragenter', onDragEnter)
      body.removeEventListener('dragleave', onDragLeave)
      body.removeEventListener('dragover', onDragOver)
      body.removeEventListener('drop', onDrop)
      if (!previousPosition) body.style.position = ''
    }
  }, [canUpload])

  // antd Upload (NOT Dragger) — gives us the file-picker button with
  // zero drag visuals. Drag-drop is owned entirely by the native
  // listener attached above.
  const handleBeforeUpload: UploadProps['beforeUpload'] = (file, fileList) => {
    const isLastFile = fileList[fileList.length - 1] === file
    if (isLastFile) {
      dispatchFiles(fileList as unknown as File[])
    }
    return false
  }

  const counterChip = (
    <Tag
      color={atCap ? 'error' : nearCap ? 'warning' : 'default'}
      aria-label={`Project file count: ${count} of ${PROJECT_FILE_CAP}`}
    >
      {count} / {PROJECT_FILE_CAP} files
    </Tag>
  )

  const uploadButton = canUpload ? (
    <Upload
      multiple
      showUploadList={false}
      beforeUpload={handleBeforeUpload}
      accept="*/*"
      disabled={atCap}
    >
      <Tooltip title={atCap ? `At ${PROJECT_FILE_CAP}-file cap` : 'Upload files'}>
        <Button
          type="primary"
          icon={<UploadOutlined />}
          disabled={atCap}
          aria-label="Upload files to project"
        >
          Upload
        </Button>
      </Tooltip>
    </Upload>
  ) : null

  const header = (
    <div className="flex items-center justify-between gap-2 mb-3 flex-wrap">
      <div className="flex items-center gap-2">
        <Typography.Text strong>Knowledge files</Typography.Text>
        {counterChip}
      </div>
      {uploadButton}
    </div>
  )

  const uploadingRows = Array.from(uploadingFiles.values())
  const uploadingPreview =
    uploadingRows.length === 0 ? null : (
      <div className="flex flex-col mb-3 gap-2">
        {uploadingRows.map(progress => (
          <div
            key={progress.id}
            className="flex items-center gap-3 px-3 py-2 rounded"
            style={{
              border: `1px solid ${
                progress.status === 'error'
                  ? token.colorErrorBorder
                  : token.colorBorderSecondary
              }`,
              backgroundColor: token.colorBgContainer,
            }}
          >
            <FileOutlined
              style={{
                color:
                  progress.status === 'error'
                    ? token.colorError
                    : token.colorIcon,
                flexShrink: 0,
              }}
            />
            <div className="flex-1 min-w-0">
              <div className="flex items-baseline justify-between gap-2">
                <Typography.Text
                  ellipsis={{ tooltip: progress.filename }}
                  className="block"
                >
                  {progress.filename}
                </Typography.Text>
                <Typography.Text
                  type="secondary"
                  className="text-xs flex-shrink-0"
                >
                  {formatFileSize(progress.size)}
                </Typography.Text>
              </div>
              {progress.status === 'error' ? (
                <Typography.Text type="danger" className="text-xs block">
                  {progress.error ?? 'Upload failed'}
                </Typography.Text>
              ) : (
                <Progress
                  percent={Math.round(progress.progress)}
                  size="small"
                  showInfo={false}
                  status={
                    progress.status === 'uploading' ? 'active' : 'normal'
                  }
                />
              )}
            </div>
            <Button
              type="text"
              icon={<CloseCircleOutlined />}
              aria-label={`Dismiss ${progress.filename}`}
              onClick={() =>
                Stores.ProjectDetail.dismissUploadingFile(progress.id)
              }
            />
          </div>
        ))}
      </div>
    )

  const emptyOrList =
    !filesLoading && files.length === 0 ? (
      <Empty
        image={Empty.PRESENTED_IMAGE_SIMPLE}
        description="No knowledge files yet"
      >
        <Typography.Text type="secondary" className="block">
          {canUpload
            ? 'Drag files anywhere on this drawer, or use the Upload button above.'
            : 'Attach files from your library to share their contents with every conversation in this project.'}
        </Typography.Text>
      </Empty>
    ) : filesLoading ? (
      <div className="flex justify-center py-6">
        <Spin />
      </div>
    ) : (
      <div className="flex flex-col">
        {files.map(file => (
          <div
            key={file.id}
            className="flex items-center gap-3 py-3"
            style={{ borderBottom: `1px solid ${token.colorSplit}` }}
          >
            <FileOutlined style={{ color: token.colorIcon, flexShrink: 0 }} />
            <div className="flex-1 min-w-0">
              <Typography.Text
                ellipsis={{ tooltip: file.filename }}
                className="block"
              >
                {file.filename}
              </Typography.Text>
              <Typography.Text type="secondary" className="text-xs">
                {file.mime_type ?? 'unknown type'}
              </Typography.Text>
            </div>
            {canEdit && (
              <Popconfirm
                title="Remove file from project?"
                description="The file itself is preserved; only its project attachment is removed."
                okText="Remove"
                okButtonProps={{ danger: true }}
                cancelText="Cancel"
                onConfirm={() => handleDetach(file)}
              >
                <Button
                  type="text"
                  danger
                  icon={<DeleteOutlined />}
                  aria-label={`Remove ${file.filename}`}
                />
              </Popconfirm>
            )}
          </div>
        ))}
      </div>
    )

  return (
    <div ref={rootRef} className={className}>
      {/* Sticky header — pins the title, count chip, Upload button,
          cap warning, and in-progress upload rows to the top of the
          scroll viewport so they stay visible while the file list
          below scrolls. `top: -1px` defeats a 1px gap that some
          browsers leave on sticky elements, and is paired with
          `paddingTop: 1px` on the inner content so visible content
          still starts at the very top. Background is the drawer
          body's `colorBgLayout` so the sticky band masks content
          scrolling underneath. */}
      <div
        className="sticky z-10 -mx-1 px-1"
        style={{
          top: -1,
          paddingTop: 1,
          backgroundColor: token.colorBgLayout,
        }}
      >
        {header}
        {atCap && (
          <Typography.Text type="danger" className="block mb-2 text-sm">
            You've reached the {PROJECT_FILE_CAP}-file cap. Remove a file to
            attach a new one.
          </Typography.Text>
        )}
        {uploadingPreview}
      </div>
      {emptyOrList}

      {/* Overlay rendered as a Portal child of `.ant-drawer-body` so
          it covers the full drawer surface (DivScrollY's internal
          viewport doesn't propagate height, so an overlay sibling of
          our content would only cover the file-list area). */}
      {isDragging && drawerBody &&
        createPortal(
          <div
            className="absolute inset-0 flex flex-col items-center justify-center gap-2"
            style={{
              zIndex: 10,
              backgroundColor: token.colorPrimaryBg,
              border: `2px dashed ${token.colorPrimary}`,
              borderRadius: 8,
              color: token.colorPrimary,
              fontWeight: 500,
              fontSize: 16,
              pointerEvents: 'none',
            }}
          >
            <UploadOutlined style={{ fontSize: 36 }} />
            <span>Drop files to attach to this project</span>
          </div>,
          drawerBody,
        )}
    </div>
  )
}
