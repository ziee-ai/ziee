import { Download as DownloadIcon, File } from 'lucide-react'
import { Button, Image, Space, Text } from '@/components/ui'
import { useEffect, useRef, useState } from 'react'
import { ApiClient } from '@/api-client'
import type { StepArtifactMeta } from '@/modules/workflow/stores/WorkflowRun.store'
import { formatBytes } from '@/utils/downloadUtils'
import { message } from '@/components/ui'

interface StepArtifactsProps {
  runId: string
  stepId: string
  artifacts: StepArtifactMeta[]
}

/**
 * Renders a step's artifacts (files written to `artifacts/<step_id>/`)
 * as attachment-style blocks (§4.7). Images (mime `image/*`) render
 * inline by fetching the bytes via
 * `GET /api/workflow-runs/{run}/artifact/{step}/{filename}` and turning
 * the Blob into an object URL; everything else renders as a download
 * chip that fetches + saves the file on click.
 */
export function StepArtifacts({
  runId,
  stepId,
  artifacts,
}: StepArtifactsProps) {
  if (artifacts.length === 0) return null
  return (
    <div className="flex flex-col gap-2">
      <Text type="secondary" className="text-xs">
        Artifacts
      </Text>
      {artifacts.map(a => (
        <ArtifactBlock
          key={a.filename}
          runId={runId}
          stepId={stepId}
          artifact={a}
        />
      ))}
    </div>
  )
}

function ArtifactBlock({
  runId,
  stepId,
  artifact,
}: {
  runId: string
  stepId: string
  artifact: StepArtifactMeta
}) {
  const isImage = (artifact.mime_type ?? '').startsWith('image/')
  const [imageUrl, setImageUrl] = useState<string | null>(null)
  const [downloading, setDownloading] = useState(false)
  const objectUrlRef = useRef<string | null>(null)

  const fetchBlob = async (): Promise<Blob> => {
    const res = await ApiClient.Workflow.readArtifact({
      run_id: runId,
      step_id: stepId,
      filename: artifact.filename,
    })
    if (res instanceof Blob) return res
    // Text / JSON content types come back already decoded; wrap so the
    // download path is uniform.
    const text = typeof res === 'string' ? res : JSON.stringify(res, null, 2)
    return new Blob([text], {
      type: artifact.mime_type || 'text/plain',
    })
  }

  useEffect(() => {
    if (!isImage) return
    let cancelled = false
    void (async () => {
      try {
        const blob = await fetchBlob()
        if (cancelled) return
        const url = URL.createObjectURL(blob)
        objectUrlRef.current = url
        setImageUrl(url)
      } catch {
        // Leave imageUrl null → falls back to the download chip.
      }
    })()
    return () => {
      cancelled = true
      if (objectUrlRef.current) {
        URL.revokeObjectURL(objectUrlRef.current)
        objectUrlRef.current = null
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [runId, stepId, artifact.filename, isImage])

  const handleDownload = async () => {
    setDownloading(true)
    try {
      const blob = await fetchBlob()
      const url = URL.createObjectURL(blob)
      const link = document.createElement('a')
      link.href = url
      link.download = artifact.filename
      document.body.appendChild(link)
      link.click()
      document.body.removeChild(link)
      URL.revokeObjectURL(url)
    } catch {
      message.error(`Failed to download ${artifact.filename}`)
    } finally {
      setDownloading(false)
    }
  }

  const meta = (
    <Space size={4}>
      <Text className="text-xs font-medium">{artifact.filename}</Text>
      {artifact.size_bytes != null && (
        <Text type="secondary" className="text-xs">
          · {formatBytes(artifact.size_bytes)}
        </Text>
      )}
      {artifact.mime_type && (
        <Text type="secondary" className="text-xs">
          · {artifact.mime_type}
        </Text>
      )}
    </Space>
  )

  if (isImage && imageUrl) {
    return (
      <div className="flex flex-col gap-1 border rounded p-2">
        {meta}
        <Image
          src={imageUrl}
          alt={artifact.filename}
          className="max-h-60 object-contain"
        />
        {artifact.description && (
          <Text type="secondary" className="text-xs">
            {artifact.description}
          </Text>
        )}
      </div>
    )
  }

  return (
    <div className="flex items-center justify-between gap-2 border rounded p-2">
      <Space size={8}>
        <File aria-hidden="true" />
        <div className="flex flex-col">
          {meta}
          {artifact.description && (
            <Text type="secondary" className="text-xs">
              {artifact.description}
            </Text>
          )}
        </div>
      </Space>
      <Button
        data-testid={`wf-artifact-download-btn-${artifact.filename}`}
        size="default"
        variant="ghost"
        icon={<DownloadIcon aria-hidden="true" />}
        loading={downloading}
        onClick={() => void handleDownload()}
        aria-label={`Download ${artifact.filename}`}
      >
        Download
      </Button>
    </div>
  )
}
