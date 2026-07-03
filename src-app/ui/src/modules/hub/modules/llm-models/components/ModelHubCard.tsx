import { LayoutGrid, Download, CircleAlert, Eye, FileText, Key, Lock, MessageSquare, Image, RotateCw, Search, Wrench } from 'lucide-react'
import {
  Card,
  Progress,
  Tag,
  Text,
  Button,
  Flex,
  Select,
  Tooltip,
  message,
  dialog,
} from '@/components/ui'
import { formatSpeed, formatTime } from '@/utils/downloadUtils'
import {
  Permissions,
  type DownloadInstance,
  type HubLocalProvider,
  type HubModel,
  type ModelQuantization,
  type ModelSource,
} from '@/api-client/types'
import { useState } from 'react'
import { ModelDetailsDrawer } from '@/modules/hub/modules/llm-models/components/ModelDetailsDrawer'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { useHubModelDownloadGate } from '@/modules/hub/modules/llm-models/hooks/useHubModelDownloadGate'

interface ModelHubCardProps {
  model: HubModel
}

export function ModelHubCard({ model }: ModelHubCardProps) {
  const [showDetails, setShowDetails] = useState(false)
  const canDownload = usePermission(Permissions.HubModelsCreate)

  // Shared pre-download gating (resolve repo → enabled? → probe).
  // The hook owns the modal lifecycle + the probe-in-flight flag,
  // so this component just wires the result into its UX.
  const { runGates, probing } = useHubModelDownloadGate()

  const { localProviders } = Stores.HubModels
  const { downloads } = Stores.LlmModelDownload

  // v2 Phase 7: repository_path moved off the model and onto each
  // source. Walk every source's identifier to match (a single model
  // may have multiple sources, but each source uses its identifier
  // as the repository_path passed to the download backend).
  const sourcePaths = (model.sources ?? []).map(s => s.identifier)

  // v2 Phase 7 auth gate: derive the required+secret env var name from
  // the first source (single-source models are the common case in the
  // current seed; a future multi-source UI would pivot on the selected
  // source index). Falls back to `null` when the model needs no auth.
  const selectedSource = model.sources?.[0]
  const requiredSecretEnvVar = selectedSource?.environmentVariables?.find(
    e => e.isRequired && e.isSecret,
  )
  const modelNeedsAuth = !!requiredSecretEnvVar
  const authEnvVarName = requiredSecretEnvVar?.name ?? null
  // Surfaced metadata: format chip + size pull from the default
  // quantization within the first source rather than v1's model-wide
  // `file_format` / `size_gb`.
  const primarySource = selectedSource
  const primarySourceDefaultQuant =
    primarySource?.quantizations.find(q => q.isDefault) ??
    primarySource?.quantizations[0]
  const displayFormat = primarySource?.fileFormat
  const displaySizeGb = primarySourceDefaultQuant?.sizeGb
  // All downloads belonging to this model (matched by repository_path),
  // partitioned by status. Precedence below: active > downloaded > failed > idle.
  // Failed entries stay in the store array (they're filtered out only on
  // cancelled/completed transitions per LlmModelDownload.store.ts), so we
  // intentionally surface them on the card with a Retry affordance.
  const downloadsForThisModel = downloads.filter(d => {
    const p = d.request_data.repository_path
    return p ? sourcePaths.includes(p) : false
  })
  const activeDownload = downloadsForThisModel.find(
    d => d.status === 'downloading' || d.status === 'pending',
  )

  const isModelBeingDownloaded = !!activeDownload

  // Check if this hub model has been downloaded (system-wide tracking via hub)
  const isModelDownloaded = (model.created_ids?.length ?? 0) > 0

  // Surface failed downloads ONLY when nothing more recent overrides them:
  // a fresh active download supersedes (the user is retrying); a successful
  // download (model.created_ids populated by loadModelsForProvider on the
  // SSE completion tick) also supersedes. Without this precedence, a stale
  // failed entry would shadow the "Downloaded" tag after a successful retry.
  const failedDownload =
    !activeDownload && !isModelDownloaded
      ? downloadsForThisModel.find(d => d.status === 'failed')
      : undefined

  const handleDownload = async (retryFrom?: DownloadInstance) => {
    // ─── Pre-download gates (resolve repo → enabled? → probe) ──────
    // The hook surfaces its own modals on gate failure (Repository
    // Disabled / Authentication Required / Cannot Connect / Repository
    // Not Configured), all of which route the primary button to the
    // LlmRepositoryDrawer. We only proceed when it returns ok=true.
    const gateResult = await runGates(model)
    if (!gateResult.ok) {
      return
    }

    // ─── Probe passed — proceed with the existing download flow ────

    if (localProviders.length === 0) {
      message.error(
        `No local provider found. Please ask an administrator to configure a local provider.`,
      )
      return
    }

    // v2 Phase 7 source + quantization defaults. For now the FE picks
    // source[0] automatically (the seed only ships single-source models
    // — a multi-source picker is a follow-up). The quantization picker
    // walks `sources[0].quantizations[]` for backward-compat with the
    // prior UX.
    const primarySource: ModelSource | undefined = model.sources?.[0]
    const sourceQuants: ModelQuantization[] =
      primarySource?.quantizations ?? []
    const defaultQuant: ModelQuantization | undefined =
      sourceQuants.find(q => q.isDefault) ?? sourceQuants[0]

    // Retry-from-failed shortcut: reuse the provider + quantization
    // the user already picked on the first attempt instead of
    // re-prompting. Without this, a Retry click silently re-opens
    // the same Select Quantization / Select Local Provider modals
    // the user already dismissed once — confusing UX and easy to
    // accidentally pick a different quant on retry. Both lookups
    // tolerate a stale ID (provider deleted / quant removed from
    // the manifest) by falling through to the modal flow.
    let provider: HubLocalProvider | undefined = localProviders[0]
    let selectedQuantization: ModelQuantization | undefined = defaultQuant

    const retryProvider = retryFrom?.provider_id
      ? localProviders.find(p => p.id === retryFrom.provider_id)
      : undefined
    const retryQuantName = retryFrom?.request_data.quantization
    const retryQuant = retryQuantName
      ? sourceQuants.find(q => q.name === retryQuantName)
      : undefined

    if (retryProvider) provider = retryProvider
    if (retryQuant) selectedQuantization = retryQuant

    const skipProviderModal = !!retryProvider
    const skipQuantModal = !!retryQuant

    // Handle quantization options selection
    if (!skipQuantModal && sourceQuants.length > 1) {
      selectedQuantization = defaultQuant ?? sourceQuants[0]

      const ok = await dialog.confirm({
        title: 'Select Quantization',
        okText: 'Continue',
        cancelText: 'Cancel',
        testid: 'hub-model-download-quant-dialog',
        description: (
          <div className="flex flex-col gap-2">
            <Text>
              Multiple quantization options available. Please select one:
            </Text>
            <Select
              data-testid="hub-model-quant-select"
              options={sourceQuants.map(option => ({
                label: (
                  <div className="flex flex-col">
                    <Text strong>{option.name.toUpperCase()}</Text>
                    <Text type="secondary" className="text-xs">
                      {option.mainFile} · {option.sizeGb} GB
                    </Text>
                  </div>
                ),
                value: option.name,
              }))}
              defaultValue={selectedQuantization?.name}
              onChange={value => {
                selectedQuantization = sourceQuants.find(
                  opt => opt.name === value,
                )
              }}
              placeholder="Select quantization"
              optionRender={option => option.label}
              labelRender={option => (
                <Text strong>
                  {String(option?.value ?? '').toUpperCase()}
                </Text>
              )}
            />
          </div>
        ),
      })

      if (!ok) {
        selectedQuantization = undefined
      }

      if (!selectedQuantization) {
        return
      }
    }

    if (!skipProviderModal && localProviders.length > 1) {
      const ok = await dialog.confirm({
        title: 'Select Local Provider',
        okText: 'Continue',
        cancelText: 'Cancel',
        testid: 'hub-model-download-provider-dialog',
        description: (
          <div className="flex flex-col gap-2">
            <Text>
              Multiple local providers found. Please select one to download
              the model:
            </Text>
            <Select
              data-testid="hub-model-provider-select"
              options={localProviders.map(p => ({
                label: p.name,
                value: p.id,
              }))}
              defaultValue={localProviders[0].id}
              onChange={value => {
                provider = localProviders.find(p => p.id === value)!
              }}
              placeholder="Select a provider"
            />
          </div>
        ),
      })

      if (!ok) {
        provider = undefined
      }
    }

    if (!provider) {
      return
    }

    try {
      const display_name = selectedQuantization
        ? `${model.display_name} (${selectedQuantization.name.toUpperCase()})`
        : model.display_name

      await Stores.HubModels.downloadModelFromHub(
        model.name,
        provider.id,
        display_name,
        selectedQuantization?.name,
        // v2 Phase 7: pin to sources[0]. A future multi-source UI
        // would surface this picker.
        0,
      )

      message.success(
        `Download started for ${model.display_name}. You can monitor the progress in the download view.`,
      )
    } catch (error: any) {
      console.error('Failed to start model download:', error)
      message.error(
        `Failed to start download for ${model.display_name}: ${error.message || 'Unknown error'}`,
      )
    }
  }

  // Same README + Download actions as the card, for the detail drawer footer.
  const hasReadme = !!(model.repository?.url || primarySource)
  const openReadme = () => {
    const fallback =
      primarySource?.registryType === 'huggingface'
        ? `https://huggingface.co/${primarySource.identifier}/blob/main/README.md`
        : model.repository?.url
    const readmeUrl = fallback ?? model.websiteUrl ?? ''
    if (readmeUrl) window.open(readmeUrl, '_blank')
  }
  const drawerFooter =
    hasReadme || (canDownload && !failedDownload) ? (
      <Flex justify="end" gap="small">
        {hasReadme && (
          <Button
            variant="outline"
            icon={<FileText />}
            data-testid={`hub-model-drawer-readme-btn-${model.name}`}
            onClick={openReadme}
          >
            README
          </Button>
        )}
        {canDownload && !failedDownload && (
          <Button
            variant="default"
            icon={<Download />}
            data-testid={`hub-model-drawer-download-btn-${model.name}`}
            onClick={() => handleDownload()}
            disabled={isModelBeingDownloaded || probing}
            loading={isModelBeingDownloaded || probing}
          >
            {probing ? 'Testing…' : 'Download'}
          </Button>
        )}
      </Flex>
    ) : undefined

  return (
    <>
      <Card
        hoverable
        className="cursor-pointer relative group hover:!shadow-md transition-shadow h-full"
        onClick={() => setShowDetails(true)}
        data-model-id={model.name}
        data-testid={`hub-model-card-${model.name}`}
      >
        <div className="flex items-start gap-3 flex-wrap">
          {/* Model Info */}
          <div className="flex-1">
            <div className="flex items-center gap-2 mb-2 flex-wrap">
              <div className="flex-1 min-w-48">
                <Flex className="gap-2 items-center flex-wrap">
                  <Text
                    className="font-medium cursor-pointer whitespace-nowrap"
                    data-testid={`hub-model-name-${model.name}`}
                  >
                    {model.display_name}
                  </Text>
                  {/* v2 per-entry version — see AssistantHubCard. */}
                  {model.version && (
                    <Tag className="text-xs !m-0" data-testid={`hub-model-version-tag-${model.name}`}>v{model.version}</Tag>
                  )}
                  {/* Top status tag — minimal, no percent (the
                      full-width bar at the bottom carries that).
                      Precedence: active > downloaded > failed. */}
                  {isModelBeingDownloaded ? (
                    <Tag tone="info" icon={<Download />} data-testid={`hub-model-status-tag-${model.name}`}>
                      Downloading
                    </Tag>
                  ) : isModelDownloaded ? (
                    <Tag tone="info" variant="solid" data-testid={`hub-model-status-tag-${model.name}`}>Downloaded</Tag>
                  ) : failedDownload ? (
                    <Tag tone="error" icon={<CircleAlert />} data-testid={`hub-model-status-tag-${model.name}`}>
                      Download Failed
                    </Tag>
                  ) : null}
                  {modelNeedsAuth && (
                    <Tooltip
                      title={
                        model.source_auth_configured
                          ? 'This model requires authentication; a credential is configured.'
                          : `This model needs ${authEnvVarName ?? 'a credential'} for its source repository. Add one in Settings → LLM Repositories before downloading.`
                      }
                    >
                      <Tag
                        data-testid={`hub-model-auth-tag-${model.name}`}
                        tone={model.source_auth_configured ? 'warning' : 'error'}
                        icon={
                          model.source_auth_configured ? (
                            <Lock />
                          ) : (
                            <Key />
                          )
                        }
                      >
                        {model.source_auth_configured
                          ? 'Auth Required'
                          : `${authEnvVarName ?? 'Token'} Needed`}
                      </Tag>
                    </Tooltip>
                  )}
                </Flex>
              </div>
              <div className="flex flex-wrap gap-1 items-center justify-end">
                {/* v2 Phase 7: link out to the source repository's
                    homepage. Prefer the per-source identifier under
                    huggingface.co; fall back to the model-level
                    `repository.url` / `website_url` if neither is set
                    (the seed always sets one). */}
                {model.repository?.url || primarySource ? (
                  <Button
                    variant="outline"
                    icon={<FileText />}
                    data-testid={`hub-model-readme-btn-${model.name}`}
                    onClick={e => {
                      e.stopPropagation()
                      const fallback =
                        primarySource?.registryType === 'huggingface'
                          ? `https://huggingface.co/${primarySource.identifier}/blob/main/README.md`
                          : model.repository?.url
                      const readmeUrl =
                        fallback ?? model.websiteUrl ?? ''
                      if (readmeUrl) {
                        window.open(readmeUrl, '_blank')
                      }
                    }}
                  >
                    README
                  </Button>
                ) : null}
                {canDownload && !failedDownload && (
                  <Button
                    icon={<Download />}
                    data-testid={`hub-model-download-btn-${model.name}`}
                    onClick={e => {
                      e.stopPropagation()
                      handleDownload()
                    }}
                    // Probing = pre-download connection test in flight
                    // (up to 10s on the upstream timeout). Disable +
                    // spinner so the user sees something is happening
                    // and can't double-click to fire concurrent probes.
                    // When a failed download is present, the Retry button
                    // under the progress bar takes over — hide the primary
                    // Download button so there's only one path forward.
                    disabled={isModelBeingDownloaded || probing}
                    loading={isModelBeingDownloaded || probing}
                  >
                    {probing ? 'Testing…' : 'Download'}
                  </Button>
                )}
              </div>
            </div>

            <div>
              {model.description && (
                <Text type="secondary" className="text-sm mb-2 block">
                  {model.description}
                </Text>
              )}

              {/* Capabilities */}
              {model.capabilities && (
                <div className="mb-2">
                  <Text type="secondary" className="text-xs mr-2">
                    Capabilities:
                  </Text>
                  <Flex wrap inline className="gap-1">
                    {model.capabilities.vision && (
                      <Tag
                        tone="info"
                        icon={<Eye />}
                        className="text-xs"
                        data-testid={`hub-model-cap-vision-tag-${model.name}`}
                      >
                        Vision
                      </Tag>
                    )}
                    {model.capabilities.tools && (
                      <Tag
                        tone="info"
                        icon={<Wrench />}
                        className="text-xs"
                        data-testid={`hub-model-cap-tools-tag-${model.name}`}
                      >
                        Tools
                      </Tag>
                    )}
                    {model.capabilities.code_interpreter && (
                      <Tag
                        tone="warning"
                        icon={<LayoutGrid />}
                        className="text-xs"
                        data-testid={`hub-model-cap-code-tag-${model.name}`}
                      >
                        Code
                      </Tag>
                    )}
                    {model.capabilities.chat && (
                      <Tag
                        tone="success"
                        icon={<MessageSquare />}
                        className="text-xs"
                        data-testid={`hub-model-cap-chat-tag-${model.name}`}
                      >
                        Chat
                      </Tag>
                    )}
                    {model.capabilities.text_embedding && (
                      <Tag
                        tone="info"
                        icon={<Search />}
                        className="text-xs"
                        data-testid={`hub-model-cap-embedding-tag-${model.name}`}
                      >
                        Embedding
                      </Tag>
                    )}
                    {model.capabilities.image_generator && (
                      <Tag
                        tone="error"
                        icon={<Image />}
                        className="text-xs"
                        data-testid={`hub-model-cap-image-tag-${model.name}`}
                      >
                        Image Gen
                      </Tag>
                    )}
                  </Flex>
                </div>
              )}

              {/* Tags */}
              {model.tags && model.tags.length > 0 && (
                <div className="mb-2">
                  <Text type="secondary" className="text-xs mr-2">
                    Tags:
                  </Text>
                  <Flex wrap inline className="gap-1">
                    {model.tags.map(tag => (
                      <Tag key={tag} className="text-xs" data-testid={`hub-model-card-tag-${model.name}-${tag}`}>
                        {tag}
                      </Tag>
                    ))}
                  </Flex>
                </div>
              )}

              {/* Metadata — pulled from sources[0]/quantizations under
                  v2 Phase 7 (model-wide `size_gb`/`file_format` gone). */}
              <div className="mb-2">
                <Flex wrap className="gap-x-4 text-xs">
                  {typeof displaySizeGb === 'number' && (
                    <span>
                      <Text type="secondary" className="text-xs">
                        Size:
                      </Text>{' '}
                      {displaySizeGb} GB
                    </span>
                  )}
                  {displayFormat && (
                    <span>
                      <Text type="secondary" className="text-xs">
                        Format:
                      </Text>{' '}
                      {displayFormat.toUpperCase()}
                    </span>
                  )}
                  {model.license && (
                    <span>
                      <Text type="secondary" className="text-xs">
                        License:
                      </Text>{' '}
                      {model.license}
                    </span>
                  )}
                  {model.author && (
                    <span>
                      <Text type="secondary" className="text-xs">
                        Author:
                      </Text>{' '}
                      {model.author}
                    </span>
                  )}
                </Flex>
              </div>
            </div>
          </div>
        </div>

        {/* Download progress / failure bar.
         *
         * Spans the full width of the card body (the wrapping
         * `<div>`s above each have padding; the Card's own
         * `body` padding bounds this). Shows EITHER:
         *   - an animated bar while a download is in flight, with
         *     `47% · 5.2 MB/s · ETA 2m 15s` style info on the right
         *   - a red error bar on failure, with the clipped error
         *     reason inline + a Retry button below
         *
         * Hidden when no download is active or failed (precedence
         * rules above + isModelDownloaded for the success case).
         */}
        {activeDownload && (
          <div
            className="mt-3"
            onClick={e => {
              // Don't open the model-details drawer when the user
              // is just trying to see the bar's progress info.
              e.stopPropagation()
            }}
          >
            <Progress
              data-testid={`hub-model-progress-${model.name}`}
              aria-label="Download progress"
              value={
                activeDownload.progress_data?.total
                  ? Math.round(
                      (activeDownload.progress_data.current /
                        activeDownload.progress_data.total) *
                        100,
                    )
                  : 0
              }
              tone="primary"
              showInfo
              format={(percent: number) => {
                const speed = activeDownload.progress_data?.speed_bps
                const eta = activeDownload.progress_data?.eta_seconds
                const parts: string[] = [`${percent ?? 0}%`]
                if (typeof speed === 'number' && speed > 0) {
                  parts.push(formatSpeed(speed))
                }
                if (typeof eta === 'number' && eta > 0) {
                  parts.push(`ETA ${formatTime(eta)}`)
                }
                return (
                  <Text className="text-xs">{parts.join(' · ')}</Text>
                )
              }}
            />
            {/* Phase / message under the bar, only when the
                backend supplies one — most downloads don't, so
                this stays hidden in the common case. */}
            {activeDownload.progress_data?.phase && (
              <Text type="secondary" className="text-xs block mt-1">
                {activeDownload.progress_data.phase}
                {activeDownload.progress_data.message
                  ? ` — ${activeDownload.progress_data.message}`
                  : ''}
              </Text>
            )}
          </div>
        )}

        {failedDownload && (
          <div
            className="mt-3"
            onClick={e => {
              e.stopPropagation()
            }}
          >
            <Tooltip
              title={failedDownload.error_message ?? 'Download failed'}
            >
              <Progress
                data-testid={`hub-model-failed-progress-${model.name}`}
                aria-label="Download progress"
                value={
                  failedDownload.progress_data?.total
                    ? Math.round(
                        ((failedDownload.progress_data.current ?? 0) /
                          failedDownload.progress_data.total) *
                          100,
                      )
                    : 0
                }
                tone="error"
                showInfo
                format={(percent: number) => {
                  const reason = failedDownload.error_message ?? 'failed'
                  // Clip the inline reason at ~50 chars; the full
                  // text lives in the wrapping Tooltip's title.
                  const shortReason =
                    reason.length > 50 ? `${reason.slice(0, 50)}…` : reason
                  return (
                    <Text className="text-xs">
                      {percent ?? 0}% — {shortReason}
                    </Text>
                  )
                }}
              />
            </Tooltip>
            {canDownload && (
              <div className="flex justify-end mt-1">
                <Button
                  variant="outline"
                  size="default"
                  icon={<RotateCw />}
                  data-testid={`hub-model-retry-btn-${model.name}`}
                  onClick={e => {
                    e.stopPropagation()
                    // Reuse the existing gates-and-probe pre-flight
                    // from handleDownload, and pass the failed
                    // DownloadInstance so the prior provider +
                    // quantization choices are preserved (no
                    // re-prompting). If the original failure was a
                    // transient repo / probe issue and the user
                    // fixed it, the retry will self-recover.
                    handleDownload(failedDownload)
                  }}
                >
                  Retry
                </Button>
              </div>
            )}
          </div>
        )}
      </Card>

      <ModelDetailsDrawer
        model={showDetails ? model : null}
        open={showDetails}
        onClose={() => setShowDetails(false)}
        footer={drawerFooter}
      />
    </>
  )
}
