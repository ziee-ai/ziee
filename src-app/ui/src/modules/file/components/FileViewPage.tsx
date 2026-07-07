// Dedicated full-page file view (route /files/:fileId). Reuses FilePanel — the
// single viewer shell — so every file type renders identically to the drawer /
// right-panel, just full-screen. Reached via the FullPageButton chrome.

import { ArrowLeft, FileQuestion } from 'lucide-react'
import { useEffect, useState } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import { ApiClient } from '@/api-client'
import type { File as FileEntity } from '@/api-client/types'
import { Button, Empty, Spin, Text } from '@/components/ui'
import { FilePanel } from '@/modules/file/components/FilePanel'

type LoadState =
  | { status: 'loading' }
  | { status: 'ready'; file: FileEntity }
  | { status: 'not-found' }

export function FileViewPage() {
  const { fileId } = useParams<{ fileId: string }>()
  const navigate = useNavigate()
  const [state, setState] = useState<LoadState>({ status: 'loading' })

  useEffect(() => {
    let cancelled = false
    if (!fileId) {
      setState({ status: 'not-found' })
      return
    }
    setState({ status: 'loading' })
    ApiClient.File.get({ file_id: fileId })
      .then(file => {
        if (!cancelled) setState({ status: 'ready', file })
      })
      .catch(() => {
        if (!cancelled) setState({ status: 'not-found' })
      })
    return () => {
      cancelled = true
    }
  }, [fileId])

  return (
    <div className="flex flex-col h-full w-full bg-background" data-testid="file-view-page">
      <div className="flex items-center gap-2 px-3 py-2 flex-shrink-0 border-border border-b">
        <Button
          variant="ghost"
          size="icon"
          tooltip="Back"
          aria-label="Back"
          icon={<ArrowLeft />}
          onClick={() => navigate(-1)}
          data-testid="file-view-back-btn"
        />
        {state.status === 'ready' ? (
          <Text strong className="truncate" title={state.file.filename}>
            {state.file.filename}
          </Text>
        ) : (
          <Text type="secondary">File</Text>
        )}
      </div>

      <div className="flex-1 overflow-hidden">
        {state.status === 'loading' ? (
          <div className="flex items-center justify-center h-full">
            <Spin label="Loading" />
          </div>
        ) : state.status === 'not-found' ? (
          <div
            className="flex flex-col items-center justify-center h-full p-6"
            data-testid="file-view-not-found"
          >
            <Empty
              data-testid="file-view-not-found-empty"
              icon={<FileQuestion className="text-5xl text-muted-foreground" />}
              description={
                <div className="flex flex-col items-center gap-1">
                  <Text strong>File not found</Text>
                  <Text type="secondary" className="text-xs">
                    This file doesn't exist or you don't have access to it.
                  </Text>
                </div>
              }
            />
          </div>
        ) : (
          // showFullPage=false — we're already the full-page view.
          <FilePanel file={state.file} showFullPage={false} />
        )}
      </div>
    </div>
  )
}
