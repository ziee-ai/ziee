import { Download, RotateCw, Upload as UploadIcon } from 'lucide-react'
import { useState } from 'react'
import {
  Permissions,
  type SnapshotDto,
  type VoiceCatalogModel,
} from '@/api-client/types'
import { ListPagination } from '@/components/common/ListPagination'
import {
  Button,
  Card,
  Empty,
  ErrorState,
  Flex,
  Input,
  message,
  Progress,
  Separator,
  Space,
  Spin,
  Tag,
  Text,
} from '@ziee/kit'
import { Can } from '@/core/permissions'
import { Stores } from '@/core/stores'
import { formatBytes } from '@/utils/downloadUtils'

const PAGE_SIZE = 10

/**
 * Downloadable whisper models — the catalog fetched from the configured source
 * repo, with a live Install progress bar driven by the SSE download store.
 * Reload-safe (VoiceModelDownloadProgress.init re-attaches to in-flight tasks).
 * Mirrors the sibling AvailableVersionsCard.
 */
export function AvailableModelsCard() {
  const { catalog, sourceReachable, sourceRepo, checking, error, hasLoaded } =
    Stores.VoiceModelUpdate
  const { activeByKey } = Stores.VoiceModelDownloadProgress
  const [page, setPage] = useState(1)

  const handleDownload = async (m: VoiceCatalogModel) => {
    try {
      await Stores.VoiceModelDownloadProgress.startDownload({ name: m.name })
    } catch (e) {
      message.error(e instanceof Error ? e.message : 'Failed to start download')
    }
  }

  const handleCheckForUpdates = async () => {
    try {
      const result = await Stores.VoiceModelUpdate.checkForUpdates()
      const newCount = (result?.models ?? []).filter(m => !m.installed).length
      if (newCount === 0) {
        message.success("No new models — you're up to date.")
      } else {
        message.success(
          `Found ${newCount} downloadable ${newCount === 1 ? 'model' : 'models'}.`,
        )
      }
    } catch (e) {
      message.error(
        e instanceof Error
          ? `Catalog check failed: ${e.message}`
          : 'Catalog check failed',
      )
    }
  }

  const total = catalog.length
  const pageItems = catalog.slice((page - 1) * PAGE_SIZE, page * PAGE_SIZE)

  return (
    <Card
      title="Available models"
      data-testid="voice-available-models-card"
      extra={
        <Flex gap="small" wrap>
          <Can permission={Permissions.VoiceAdminManage}>
            <Button
              icon={<UploadIcon />}
              variant="outline"
              onClick={() =>
                Stores.VoiceUploadModelDrawer.openUploadModelDrawer()
              }
              data-testid="voice-model-upload-open-btn"
              aria-label="Upload a model file"
            >
              Upload
            </Button>
          </Can>
          <Can permission={Permissions.VoiceAdminRead}>
            <Button
              icon={<RotateCw />}
              loading={checking}
              onClick={handleCheckForUpdates}
              data-testid="voice-model-check-updates-btn"
              aria-label="Check for updates"
            >
              Check for updates
            </Button>
          </Can>
        </Flex>
      }
    >
      <Flex vertical className="gap-4">
        {sourceRepo && (
          <div data-testid="voice-model-source-row">
            <Text type="secondary">Source: </Text>
            <Text strong>{sourceRepo}</Text>
          </div>
        )}

        <Can permission={Permissions.VoiceAdminManage}>
          <AddFromUrlForm />
        </Can>

        <Separator className="!my-2" />

        {checking && !hasLoaded ? (
          <Spin label="Loading catalog" />
        ) : error && !hasLoaded ? (
          <ErrorState
            resource="model catalog"
            description="Couldn't reach the model source."
            details={error}
            onRetry={() =>
              void Stores.VoiceModelUpdate.checkForUpdates().catch(() => {
                /* non-fatal */
              })
            }
            data-testid="voice-available-models-error"
          />
        ) : !sourceReachable ? (
          <Empty
            description={`The model source${sourceRepo ? ` (${sourceRepo})` : ''} is unreachable. Check your connection or repoint the source below.`}
            data-testid="voice-available-models-unreachable"
          />
        ) : total === 0 ? (
          <Empty
            description="No downloadable models found in the source."
            data-testid="voice-available-models-empty"
          />
        ) : (
          <>
            <Flex vertical gap="small">
              {pageItems.map(m => (
                <AvailableModelRow
                  key={m.name}
                  model={m}
                  progress={activeByKey.get(downloadKey(activeByKey, m.name))}
                  onDownload={() => handleDownload(m)}
                />
              ))}
            </Flex>
            {total > PAGE_SIZE && (
              <ListPagination
                current={page}
                total={total}
                pageSize={PAGE_SIZE}
                onChange={setPage}
                onPageSizeChange={() => setPage(1)}
                itemNoun="models"
                data-testid="voice-available-models-pagination"
                aria-label="Available models pagination"
              />
            )}
          </>
        )}
      </Flex>
    </Card>
  )
}

/**
 * Best-effort match of an in-flight download snapshot to a catalog model. The
 * download key is server-assigned; snapshots carry the model `name`, so match on
 * that.
 */
function downloadKey(
  activeByKey: Map<string, SnapshotDto>,
  name: string,
): string {
  for (const [key, snap] of activeByKey) {
    if (snap.name === name) return key
  }
  return ''
}

function AvailableModelRow({
  model,
  progress,
  onDownload,
}: {
  model: VoiceCatalogModel
  progress?: SnapshotDto
  onDownload: () => void
}) {
  const inProgress =
    progress != null &&
    progress.status !== 'completed' &&
    progress.status !== 'failed'
  const failed = progress?.status === 'failed'
  return (
    <div
      className="rounded -mx-2 px-2 -my-1 py-1"
      data-testid={`voice-available-model-row-${model.name}`}
    >
      <Flex vertical gap="small">
        <Flex justify="between" align="center" gap="small" wrap>
          <Space wrap>
            <Text strong>{model.name}</Text>
            {model.size_bytes != null && !model.installed && (
              <Text type="secondary" className="text-xs">
                {formatBytes(model.size_bytes)}
              </Text>
            )}
            {model.quantization && (
              <Tag
                variant="outline"
                data-testid={`voice-available-model-quant-${model.name}`}
              >
                {model.quantization}
              </Tag>
            )}
            <Tag
              tone="info"
              variant="outline"
              data-testid={`voice-available-model-lang-${model.name}`}
            >
              {model.english_only ? 'English' : 'multilingual'}
            </Tag>
            {model.sha256 && (
              <Tag
                tone="success"
                variant="outline"
                data-testid={`voice-available-model-verifiable-${model.name}`}
              >
                verifiable
              </Tag>
            )}
            {model.installed && (
              <Tag
                tone="success"
                variant="outline"
                data-testid={`voice-available-model-installed-tag-${model.name}`}
              >
                installed
              </Tag>
            )}
          </Space>
          <Can permission={Permissions.VoiceAdminManage}>
            <Button
              icon={<Download />}
              loading={inProgress}
              disabled={model.installed || inProgress}
              onClick={onDownload}
              data-testid={`voice-available-model-install-${model.name}`}
              aria-label={`Install ${model.name}`}
            >
              {model.installed
                ? 'Installed'
                : inProgress
                  ? 'Installing…'
                  : 'Install'}
            </Button>
          </Can>
        </Flex>
        {progress && <DownloadProgressLine progress={progress} />}
        {failed && progress?.error && (
          <Text type="secondary">{progress.error}</Text>
        )}
      </Flex>
    </div>
  )
}

function DownloadProgressLine({ progress }: { progress: SnapshotDto }) {
  const total = progress.total_bytes ?? 0
  const recv = progress.bytes_received
  const pct =
    progress.status === 'completed'
      ? 100
      : progress.percent != null
        ? Math.round(progress.percent)
        : total > 0
          ? Math.round((recv / total) * 100)
          : undefined
  return (
    <Flex vertical className="gap-1">
      <Progress
        value={pct ?? 0}
        data-testid={`voice-model-download-progress-${progress.key}`}
        tone={
          progress.status === 'failed'
            ? 'error'
            : progress.status === 'completed'
              ? 'success'
              : 'primary'
        }
        showInfo={pct != null}
        size="sm"
        aria-label={`Download progress: ${pct ?? 0}%`}
      />
      <Text type="secondary" className="text-xs">
        {formatBytes(recv)}
        {total > 0 ? ` / ${formatBytes(total)}` : ''}
        {progress.status === 'completed' ? ' — Completed' : ''}
      </Text>
    </Flex>
  )
}

/**
 * Add a model from a raw https URL or an `owner/repo` + filename. Downloads
 * from an arbitrary source can't be digest-verified against the catalog, so the
 * result is flagged unverified.
 */
function AddFromUrlForm() {
  const [url, setUrl] = useState('')
  const [repository, setRepository] = useState('')
  const [filename, setFilename] = useState('')
  const [name, setName] = useState('')

  const canStart =
    name.trim().length > 0 &&
    (url.trim().length > 0 || filename.trim().length > 0)

  const handleAdd = async () => {
    try {
      await Stores.VoiceModelDownloadProgress.startDownload({
        name: name.trim(),
        url: url.trim() || undefined,
        repository: repository.trim() || undefined,
        filename: filename.trim() || undefined,
      })
      message.success('Download started')
      setUrl('')
      setRepository('')
      setFilename('')
      setName('')
    } catch (e) {
      message.error(e instanceof Error ? e.message : 'Failed to start download')
    }
  }

  return (
    <div
      className="rounded border border-border p-3"
      data-testid="voice-model-add-url-form"
    >
      <Flex vertical gap="small">
        <Text strong className="text-xs">
          Add from URL or Hugging Face
        </Text>
        <Flex gap="small" wrap>
          <Input
            className="flex-1 min-w-48"
            value={name}
            onChange={e => setName(e.target.value)}
            placeholder="Model name (e.g. large-v3)"
            data-testid="voice-model-add-name"
            aria-label="Model name"
          />
          <Input
            className="flex-1 min-w-48"
            value={url}
            onChange={e => setUrl(e.target.value)}
            placeholder="https://… (raw file URL)"
            data-testid="voice-model-add-url"
            aria-label="Model file URL"
          />
        </Flex>
        <Flex gap="small" wrap>
          <Input
            className="flex-1 min-w-48"
            value={repository}
            onChange={e => setRepository(e.target.value)}
            placeholder="owner/repo (optional)"
            data-testid="voice-model-add-repository"
            aria-label="Hugging Face repository"
          />
          <Input
            className="flex-1 min-w-48"
            value={filename}
            onChange={e => setFilename(e.target.value)}
            placeholder="filename in repo (e.g. ggml-large-v3.bin)"
            data-testid="voice-model-add-filename"
            aria-label="Filename in repository"
          />
        </Flex>
        <Flex justify="between" align="center" gap="small" wrap>
          <Text type="secondary" className="text-xs">
            Models added this way can't be digest-verified against the catalog
            and are marked unverified.
          </Text>
          <Button
            onClick={handleAdd}
            disabled={!canStart}
            data-testid="voice-model-add-submit"
            aria-label="Start download from URL or repository"
          >
            Add
          </Button>
        </Flex>
      </Flex>
    </div>
  )
}
