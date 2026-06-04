// Full project knowledge-files management panel — relocated from
// `projects/components/ProjectFilesPanel.tsx` and retrofitted to use
// FileCard's row variant + multi-select + the file-module's
// ProjectFiles store.
//
// Rendered inside the knowledge drawer by `ProjectKnowledgeSection`
// via the `ProjectExtensionSlot view="managePanel"` slot — the projects
// module never imports this file directly.

import { useEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import {
  App,
  Button,
  Empty,
  Popconfirm,
  Spin,
  Tag,
  Tooltip,
  Typography,
  Upload,
  theme,
} from 'antd'
import type { UploadProps } from 'antd'
import {
  DeleteOutlined,
  UploadOutlined,
} from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import { FileCard } from '@/modules/file/components/FileCard'

/**
 * Server-enforced cap (`PROJECT_MAX_FILES`). Mirrored here so the UI
 * shows a counter + warning before hitting a 422.
 */
const PROJECT_FILE_CAP = 100

/** 100 MiB — mirrors the file module's per-file cap. */
const MAX_FILE_SIZE = 100 * 1024 * 1024

function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  if (bytes < 1024 * 1024 * 1024)
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`
}

export function ProjectFilesManagePanel() {
  // `modal` from App.useApp() — NOT the static `Modal.confirm` from
  // antd. Static Modal calls render OUTSIDE the ConfigProvider's
  // context tree, so they ignore the active theme token. In dark
  // mode that surfaces as a white modal on a dark page. App.useApp()'s
  // modal instance is wired through the running context and inherits
  // the dark/light tokens correctly.
  const { message, modal } = App.useApp()
  const { token } = theme.useToken()
  const project = Stores.ProjectDetail.project
  const {
    files,
    filesLoading,
    uploadingFiles,
    selectedFileIds,
  } = Stores.ProjectFiles
  const canEdit = usePermission(Permissions.ProjectsEdit)
  const canUpload = canEdit && usePermission(Permissions.FilesUpload)

  const projectId = project?.id
  const count = files.length
  const atCap = count >= PROJECT_FILE_CAP
  const nearCap = count >= PROJECT_FILE_CAP - 5 && !atCap

  const rootRef = useRef<HTMLDivElement>(null)
  const [drawerBody, setDrawerBody] = useState<HTMLElement | null>(null)
  const [isDragging, setIsDragging] = useState(false)

  const handleDelete = async (fileId: string, filename: string) => {
    if (!projectId) return
    try {
      await Stores.ProjectFiles.deleteFile(projectId, fileId)
      message.success(`Deleted ${filename}`)
    } catch (err) {
      message.error(
        err instanceof Error ? err.message : 'Failed to delete file',
      )
    }
  }

  const handleBatchDelete = () => {
    if (!projectId || selectedFileIds.size === 0) return
    const n = selectedFileIds.size
    modal.confirm({
      title: `Delete ${n} file${n === 1 ? '' : 's'}?`,
      content: 'This permanently removes the files from your library.',
      okText: 'Delete',
      okButtonProps: { danger: true },
      cancelText: 'Cancel',
      onOk: async () => {
        try {
          await Stores.ProjectFiles.batchDelete(projectId)
          message.success(`Deleted ${n} file${n === 1 ? '' : 's'}`)
        } catch (err) {
          message.error(
            err instanceof Error ? err.message : 'Batch delete failed',
          )
        }
      },
    })
  }

  // Pre-flight + dispatch — stable ref so the DOM listener attached in
  // useEffect always calls the latest version.
  const dispatchFiles = (incoming: File[]) => {
    if (!projectId || incoming.length === 0) return
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
    void Stores.ProjectFiles.uploadAndAttachFiles(projectId, accepted)
  }
  const dispatchRef = useRef(dispatchFiles)
  dispatchRef.current = dispatchFiles

  // Drag handlers — attach to the antd Drawer's body so the overlay
  // covers the full drawer surface.
  useEffect(() => {
    if (!canUpload) return
    const body = rootRef.current?.closest(
      '.ant-drawer-body',
    ) as HTMLElement | null
    if (!body) return
    setDrawerBody(body)

    const previousPosition = body.style.position
    if (!body.style.position && getComputedStyle(body).position === 'static') {
      body.style.position = 'relative'
    }

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

  const handleBeforeUpload: UploadProps['beforeUpload'] = (file, fileList) => {
    const isLastFile = fileList[fileList.length - 1] === file
    if (isLastFile) {
      dispatchFiles(fileList as unknown as File[])
    }
    return false
  }

  if (!project) return null

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

  const selectionBar = selectedFileIds.size > 0 && (
    <div
      className="flex items-center justify-between gap-2 mb-3 px-3 py-2 rounded"
      style={{
        backgroundColor: token.colorInfoBg,
        border: `1px solid ${token.colorInfoBorder}`,
      }}
    >
      <Typography.Text>
        {selectedFileIds.size} selected
      </Typography.Text>
      <div className="flex items-center gap-2">
        <Button size="small" onClick={() => Stores.ProjectFiles.deselectAll()}>
          Clear
        </Button>
        <Button
          size="small"
          danger
          icon={<DeleteOutlined />}
          onClick={handleBatchDelete}
        >
          Delete selected
        </Button>
      </div>
    </div>
  )

  const uploadingRows = Array.from(uploadingFiles.values())
  const uploadingPreview =
    uploadingRows.length === 0 ? null : (
      <div className="flex flex-col mb-3 gap-2">
        {uploadingRows.map(progress => (
          <FileCard
            key={progress.id}
            uploadProgress={{
              ...progress,
              status: progress.status === 'pending' ? 'pending' : progress.status,
            }}
            variant="row"
            onRemove={() => Stores.ProjectFiles.dismissUploadingFile(progress.id)}
          />
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
      <div className="flex flex-col gap-2">
        {files.map(file => {
          const isSelected = selectedFileIds.has(file.id)
          return (
            <FileCard
              key={file.id}
              file={file}
              variant="row"
              canRemove={false}
              selectable={canEdit}
              selected={isSelected}
              onSelectChange={() => Stores.ProjectFiles.toggleSelection(file.id)}
              subtitle={
                <>
                  {formatFileSize(file.file_size)} · {file.mime_type ?? 'unknown'}
                </>
              }
              actions={
                canEdit ? (
                  <Popconfirm
                    title="Delete this file?"
                    description="This permanently removes the file from your library."
                    okText="Delete"
                    okButtonProps={{ danger: true }}
                    cancelText="Cancel"
                    onConfirm={() => handleDelete(file.id, file.filename)}
                  >
                    <Tooltip title="Delete">
                      <Button
                        type="text"
                        danger
                        icon={<DeleteOutlined />}
                        aria-label={`Delete ${file.filename}`}
                      />
                    </Tooltip>
                  </Popconfirm>
                ) : undefined
              }
            />
          )
        })}
      </div>
    )

  return (
    <div ref={rootRef} className="w-full">
      {/* Sticky header — keeps title/counter/upload/selection visible
          while the file list scrolls. */}
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
        {selectionBar}
        {uploadingPreview}
      </div>
      {emptyOrList}

      {/* Drag overlay portaled into the drawer body so it covers the
          full surface. */}
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
