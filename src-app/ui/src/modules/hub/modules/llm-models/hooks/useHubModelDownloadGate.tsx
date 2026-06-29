/**
 * Shared pre-download gating for hub models.
 *
 * Resolves the source `LlmRepository` from the model's
 * `repository_url`, then runs two gates:
 *
 *   1. Repository must be `enabled`. Otherwise → "Repository Disabled"
 *      modal whose primary button opens the LlmRepositoryDrawer for
 *      that repo.
 *   2. Connection probe (via `Stores.LlmRepository.testLlmRepositoryById`)
 *      must succeed. On failure, branches by whether auth is the
 *      likely culprit (`model.auth_required && !source_auth_configured`)
 *      and shows either "Authentication Required" or "Cannot Connect
 *      to Repository" — both modals' primary buttons also open the
 *      drawer for that repo.
 *
 * Both the hub model card (`ModelHubCard`) AND the sidebar download
 * widget (`DownloadIndicatorWidget`) call this hook so the retry
 * affordance is consistent across surfaces — clicking Retry from the
 * widget reliably surfaces the same modal flow the user would see if
 * they'd retried from the hub card itself.
 *
 * Modal copy + drawer-open primary buttons mirror the original
 * inline implementation in `ModelHubCard.tsx`. The hook returns:
 *
 *   { ok: true, repo }  — proceed with download
 *   { ok: false }       — a gate modal opened; caller must NOT
 *                         continue with the download
 *
 * Callers also receive a `probing` flag they can wire into a button
 * loading state, since the probe takes up to the HTTP timeout (~10s).
 */

import { Text, dialog, message } from '@/components/ui'
import { useState } from 'react'
import { Stores } from '@/core/stores'
import type { HubModel, LlmRepository } from '@/api-client/types'

/**
 * Module-scope guard so a Retry click in the sidebar widget can't
 * stack a second gate modal on top of one the hub card already
 * opened (or vice versa). A per-hook `useRef` would only protect
 * inside the same hook instance — each call site gets its own
 * instance. The module-scope flag is observed by every
 * `useHubModelDownloadGate()` consumer.
 *
 * NOT React state — the value doesn't need to participate in
 * rendering; it's a transient lock that flips on modal open and
 * off on close.
 */
let gateModalOpen = false

export interface GateRunResult {
  ok: boolean
  /** Present when `ok === true` — the resolved repo row. */
  repo?: LlmRepository
}

export function useHubModelDownloadGate() {
  const [probing, setProbing] = useState(false)

  const showRepoGateModal = (
    title: string,
    body: React.ReactNode,
    _repository: LlmRepository,
  ) => {
    if (gateModalOpen) return
    gateModalOpen = true
    dialog.info({
      title,
      description: body,
      okText: 'OK',
    }).then(() => {
      gateModalOpen = false
    })
  }

  const showRepoDisabledModal = (model: HubModel, repository: LlmRepository) =>
    showRepoGateModal(
      'Repository Disabled',
      <Text>
        Downloading <Text strong>{model.display_name}</Text> requires the{' '}
        <Text strong>{repository.name}</Text> repository to be enabled. Open
        its settings and turn it on, then try again.
      </Text>,
      repository,
    )

  const showAuthRequiredModal = (
    model: HubModel,
    repository: LlmRepository,
  ) =>
    showRepoGateModal(
      'Authentication Required',
      <Text>
        Downloading <Text strong>{model.display_name}</Text> needs a credential
        for the <Text strong>{repository.name}</Text> repository, which isn't
        configured yet. Open the repository's settings and add one, then try
        again.
      </Text>,
      repository,
    )

  const showCannotConnectModal = (
    model: HubModel,
    repository: LlmRepository,
    reason: string | undefined,
  ) =>
    showRepoGateModal(
      'Cannot Connect to Repository',
      <>
        <Text>
          The connection test for <Text strong>{repository.name}</Text> failed,
          so we can't start the download for{' '}
          <Text strong>{model.display_name}</Text>. Open the repository's
          settings to review the URL / credential and retry.
        </Text>
        {reason && (
          <Text type="secondary" className="text-xs block">
            Reason: {reason}
          </Text>
        )}
      </>,
      repository,
    )

  const showRepoNotConfiguredModal = (_model: HubModel, registryUrl: string) => {
    if (gateModalOpen) return
    gateModalOpen = true
    dialog.info({
      title: 'Repository Not Configured',
      description: (
        <Text>
          No installed repository matches the source URL{' '}
          <Text code>{registryUrl}</Text>. Add it in Settings → LLM
          Repositories before downloading.
        </Text>
      ),
      okText: 'OK',
    }).then(() => {
      gateModalOpen = false
    })
  }

  const runGates = async (model: HubModel): Promise<GateRunResult> => {
    // The LlmRepository store loads via `__init__.repositories` on
    // FIRST proxy access, but this gate hook reads via `__state` (we
    // run from event handlers — see `feedback_stores_state_in_handlers`).
    // `__state` doesn't trigger the lazy load, so if no other surface
    // has touched the store yet (e.g. a fresh session that goes
    // straight from /setup to /hub without visiting LLM Repositories),
    // `repositories` is `[]` and every gate check 404s with "Repository
    // Not Configured" even though the seed migration ships HuggingFace
    // + GitHub. Call the load action explicitly here — it's idempotent
    // via the store's `isInitialized` guard, so it's a no-op when the
    // store is already populated.
    await Stores.LlmRepository.loadLlmRepositories()
    // Snapshot the current repositories list via `.__state` — this
    // function is invoked from event handlers (Download click in the
    // hub card, Retry click in the download widget), NOT from a React
    // render path. The bare proxy access would call React hooks
    // outside render. See `feedback_stores_state_in_handlers` in
    // project memory.
    const { repositories } = Stores.LlmRepository.__state

    // ── Resolve repo ────────────────────────────────────────────────
    // v2 Phase 7: derive the registry URL from sources[0] rather than
    // the dropped model-wide `repository_url`. Keep in lockstep with
    // the backend's `derive_registry_url` in `hub/handlers.rs` —
    // otherwise the FE gate could pass while the backend lookup 404s
    // (or vice versa).
    const selectedSource = model.sources?.[0]
    const registryUrl =
      selectedSource?.registryType === 'huggingface'
        ? 'https://huggingface.co'
        : selectedSource?.registryType === 's3'
          ? 'https://s3.amazonaws.com'
          : selectedSource?.registryType === 'url'
            ? selectedSource.identifier
            : null
    if (!registryUrl) {
      showRepoNotConfiguredModal(model, model.repository?.url ?? '(unknown)')
      return { ok: false }
    }
    const repository = repositories.find(r => r.url === registryUrl)
    if (!repository) {
      showRepoNotConfiguredModal(model, registryUrl)
      return { ok: false }
    }
    const needsAuth = !!selectedSource?.environmentVariables?.find(
      e => e.isRequired && e.isSecret,
    )

    // ── Gate 1: enabled ────────────────────────────────────────────
    if (!repository.enabled) {
      showRepoDisabledModal(model, repository)
      return { ok: false }
    }

    // ── Gate 2: cross-surface probe mutual exclusion ────────────────
    // The store's `testing` flag is a singleton boolean — the same
    // probe in flight from the System Settings list-page would also
    // set it. Calling `testLlmRepositoryById` while another probe is
    // running returns a sentinel `success: false / message: '...in
    // progress'` that would otherwise drop us into the Cannot Connect
    // modal with a misleading error. Skip the duplicate probe and
    // surface a brief info toast — the user can re-click once the
    // other surface settles.
    if (Stores.LlmRepository.__state.testing) {
      message.info(
        'Connection test already running — try again in a moment.',
      )
      return { ok: false }
    }

    // ── Gate 2: connection probe ──────────────────────────────────
    setProbing(true)
    let probeResult: { success: boolean; message: string }
    try {
      probeResult = await Stores.LlmRepository.testLlmRepositoryById(
        repository.id,
        {},
      )
    } catch (error: any) {
      setProbing(false)
      showCannotConnectModal(model, repository, error?.message)
      return { ok: false }
    }
    setProbing(false)

    // Belt-and-suspenders for the in-progress sentinel: even with the
    // pre-check above, a race between the pre-check read and the
    // store mutation can let the call land while another probe is
    // running. The store returns success=false with this exact
    // string; treat it as "no result" (toast + skip), not "probe
    // failed" (which would surface Cannot Connect / Auth Required).
    if (
      !probeResult.success &&
      probeResult.message?.includes('already in progress')
    ) {
      message.info(
        'Connection test already running — try again in a moment.',
      )
      return { ok: false }
    }

    if (!probeResult.success) {
      if (needsAuth && !model.source_auth_configured) {
        showAuthRequiredModal(model, repository)
      } else {
        showCannotConnectModal(model, repository, probeResult.message)
      }
      return { ok: false }
    }

    return { ok: true, repo: repository }
  }

  return { runGates, probing }
}
