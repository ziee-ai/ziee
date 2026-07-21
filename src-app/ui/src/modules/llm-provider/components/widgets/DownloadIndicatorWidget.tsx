import { useState } from 'react'
import { X, Download, RotateCw } from 'lucide-react'
import { Badge, Button, Flex, Popover, Tooltip, message } from '@ziee/kit'
import { Stores } from '@ziee/framework/stores'
import { DownloadItem } from '@/modules/llm-provider/components/downloads/DownloadItem'
import { useHubModelDownloadGate } from '@/modules/hub/modules/llm-models/hooks/useHubModelDownloadGate'
import type {
  DownloadInstance,
  DownloadFromRepositoryRequest,
  FileFormat,
} from '@/api-client/types'
import { LlmModelDownload } from '@/modules/llm-provider/stores/llmModelDownload'

/**
 * Rebuild a `DownloadFromRepositoryRequest` from a failed
 * `DownloadInstance` so the user can retry directly from the widget
 * popover. The instance's `request_data` already carries every field
 * the request needs; we just rename `model_name` → `name` and assert
 * the required fields are present.
 *
 * Returns `null` if any required field is missing — defensive guard
 * for legacy rows or future schema drift.
 */
const KNOWN_FILE_FORMATS: ReadonlyArray<FileFormat> = [
  'safetensors',
  'pytorch',
  'gguf',
]

function buildRetryRequest(
  d: DownloadInstance,
): DownloadFromRepositoryRequest | null {
  const r = d.request_data
  if (
    !d.provider_id ||
    !d.repository_id ||
    !r.model_name ||
    !r.repository_path ||
    !r.file_format ||
    !r.main_filename ||
    !r.display_name
  ) {
    return null
  }
  // Validate file_format against the literal union — `as FileFormat`
  // would let `'pytorch_legacy'` or any future enum drift through and
  // the backend would 400 with a confusing "invalid file_format" once
  // the retry round-trips. Reject up-front with a clear toast.
  if (!KNOWN_FILE_FORMATS.includes(r.file_format as FileFormat)) {
    return null
  }
  return {
    provider_id: d.provider_id,
    repository_id: d.repository_id,
    name: r.model_name,
    repository_path: r.repository_path,
    file_format: r.file_format as FileFormat,
    main_filename: r.main_filename,
    display_name: r.display_name,
    description: r.description,
    capabilities: r.capabilities,
    engine_settings: r.engine_settings,
    engine_type: r.engine_type,
    parameters: r.parameters,
    repository_branch: r.revision,
  }
}

export function DownloadIndicatorWidget() {
  const { downloads } = LlmModelDownload
  // Same gating used by the hub model card. Sharing this means the
  // Retry button surfaces the same Repository Disabled / Auth Required
  // / Cannot Connect modals the user would see clicking Retry from the
  // hub page — failure-recovery UX stays consistent across surfaces.
  const { runGates } = useHubModelDownloadGate()
  const [popoverOpen, setPopoverOpen] = useState(false)

  // Filter for active downloads
  const activeDownloads = downloads.filter(
    (download: DownloadInstance) =>
      download.status === 'downloading' || download.status === 'pending',
  )
  const failedDownloads = downloads.filter(
    (download: DownloadInstance) => download.status === 'failed',
  )

  // Hide widget if no active or failed downloads
  if (activeDownloads.length === 0 && failedDownloads.length === 0) {
    return null
  }

  // Badge keeps active count only — including failures would be
  // confusing during a successful concurrent download. Color flips to
  // red as a "needs attention" signal when failures are present.
  const badgeCount = activeDownloads.length
  const badgeTone = failedDownloads.length > 0 ? 'error' : 'info'

  const handleRetry = async (d: DownloadInstance) => {
    // ── Preferred path: matching hub model + full gate flow ─────────
    // Look up the catalog entry by `repository_path`. When found, we
    // call the same `downloadModelFromHub` action the hub card uses,
    // gated by the SAME enabled→probe→branch flow. That way a retry
    // from the widget surfaces the Repository Disabled / Auth Required
    // / Cannot Connect modals if those gates would block — matches
    // what the user would see retrying from the hub page itself.
    //
    // ── Fallback path: low-level repo download ───────────────────────
    // Catalog drift (the hub_id was removed in a hub release between
    // start + failure) or non-hub downloads (added by some other UI
    // path) won't find a matching model. In that case we fall back to
    // the gateless `downloadLlmModelFromRepository` — at least the
    // user gets a retry; if it fails for a credentials reason, the
    // toast carries the backend's error message.
    const repoPath = d.request_data.repository_path
    // Snapshot via `.$` — `handleRetry` is an event handler,
    // not a render path. The bare `Stores.HubModels.models` proxy
    // would call React hooks outside render. See
    // `feedback_stores_state_in_handlers` in project memory.
    // v2 Phase 7: walk every source's identifier (the source
    // identifier is what the backend passes as `repository_path` to
    // the download path; matching against ALL of them lets a model
    // with multiple sources still be detected on retry).
    const hubModel = repoPath
      ? Stores.HubModels.$.models.find(m =>
          (m.sources ?? []).some(s => s.identifier === repoPath),
        )
      : undefined

    // Order: download-first, then delete the old failed row. The
    // previous order (delete → download) erased the only record of
    // request_data BEFORE the retry API call; if the POST then failed
    // (network blip, transient backend 500, gate modal interrupt),
    // the user had nothing to retry from. Now the failed row stays
    // until we have a successful start; on success we delete it; on
    // failure we leave it so Retry stays clickable.
    if (hubModel) {
      const gateResult = await runGates(hubModel)
      if (!gateResult.ok) {
        // Gate hook surfaced its own modal; nothing else to do.
        return
      }
      try {
        await Stores.HubModels.downloadModelFromHub(
          hubModel.name,
          d.provider_id,
          d.request_data.display_name ?? hubModel.display_name,
          d.request_data.quantization ?? undefined,
        )
        // Best-effort cleanup of the old failed row AFTER the new
        // download starts. If this fails (transient DB error), the
        // popover briefly shows two rows but that's a UX nit, not a
        // data-loss bug — the new download supersedes visually.
        try {
          await LlmModelDownload.deleteLlmModelDownload(d.id)
        } catch {
          // ignore — the new download visually supersedes anyway
        }
        message.success(
          `Retrying download: ${d.request_data.display_name ?? hubModel.display_name}`,
        )
      } catch (error) {
        message.error(
          error instanceof Error ? `Retry failed: ${error.message}` : 'Retry failed',
        )
      }
      return
    }

    // Fallback — catalog match missing.
    const req = buildRetryRequest(d)
    if (!req) {
      message.error(
        'This download is missing required metadata; reinstall from the hub instead.',
      )
      return
    }
    try {
      await LlmModelDownload.downloadLlmModelFromRepository(req)
      try {
        await LlmModelDownload.deleteLlmModelDownload(d.id)
      } catch {
        // ignore — new download visually supersedes
      }
      message.success(`Retrying download: ${req.display_name}`)
    } catch (error) {
      message.error(
        error instanceof Error ? `Retry failed: ${error.message}` : 'Retry failed',
      )
    }
  }

  const handleClear = async (d: DownloadInstance) => {
    try {
      await LlmModelDownload.deleteLlmModelDownload(d.id)
    } catch (error) {
      message.error(
        error instanceof Error ? error.message : 'Failed to clear download',
      )
    }
  }

  const popoverContent = (
    <div style={{ width: 320, maxHeight: 440, overflowY: 'auto' }}>
      {activeDownloads.length > 0 && (
        <>
          <strong className="block mb-3">
            Active Downloads ({activeDownloads.length})
          </strong>
          {activeDownloads.map(download => (
            <DownloadItem
              key={download.id}
              download={download}
              mode="minimal"
            />
          ))}
        </>
      )}
      {failedDownloads.length > 0 && (
        <>
          <strong
            className={`block mb-3 text-destructive${activeDownloads.length > 0 ? ' mt-4' : ''}`}
          >
            Failed Downloads ({failedDownloads.length})
          </strong>
          {failedDownloads.map(download => (
            <div key={download.id} className="mb-2">
              <DownloadItem download={download} mode="minimal" />
              <Flex justify="end" gap="sm" className="mt-1">
                <Tooltip title="Dismiss this failed download">
                  <Button
                    size="default"
                    variant="outline"
                    icon={<X />}
                    onClick={() => handleClear(download)}
                    data-testid={`llm-download-clear-btn-${download.id}`}
                  >
                    Clear
                  </Button>
                </Tooltip>
                <Tooltip title="Start a new download with the same settings">
                  <Button
                    size="default"
                    variant="default"
                    icon={<RotateCw />}
                    onClick={() => handleRetry(download)}
                    data-testid={`llm-download-retry-btn-${download.id}`}
                  >
                    Retry
                  </Button>
                </Tooltip>
              </Flex>
            </div>
          ))}
        </>
      )}
    </div>
  )

  return (
    <Popover
      content={popoverContent}
      title="Downloads"
      trigger="click"
      side="right"
      align="end"
      open={popoverOpen}
      onOpenChange={setPopoverOpen}
    >
      <div
        style={{
          padding: '12px 16px',
          cursor: 'pointer',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
        }}
      >
        <Badge
          count={badgeCount}
          tone={badgeTone}
          offset={[10, 0]}
          aria-label={`${badgeCount} active download${badgeCount !== 1 ? 's' : ''}`}
          data-testid="llm-download-indicator-badge"
        >
          <Download style={{ fontSize: 20 }} aria-label="Downloads" />
        </Badge>
      </div>
    </Popover>
  )
}
