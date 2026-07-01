import { useEffect, useRef, useState } from 'react'
import {
  Alert,
  Button,
  Card,
  Dialog,
  Flex,
  Form,
  FormField,
  InputNumber,
  Paragraph,
  Select,
  Switch,
  message,
  useForm,
} from '@/components/ui'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { SettingsFormActions } from '@/modules/settings/components/SettingsFormActions'
import { Permissions } from '@/api-client/types'
import { SettingsSectionStatus } from '@/components/common/SettingsSectionStatus'

const READ_PERM = Permissions.MemoryAdminRead
const MANAGE_PERM = Permissions.MemoryAdminManage

// Mirrors the backend `VALID_FTS_DICTIONARIES` const + the CHECK
// constraint on `memory_admin_settings.fts_dictionary`. Adding a
// language here without also extending the backend allowlist will
// surface as a 400 VALIDATION_ERROR on save.
const DICTIONARY_OPTIONS = [
  { value: 'simple', label: 'simple — language-agnostic, no stemming' },
  { value: 'english', label: 'english' },
  { value: 'french', label: 'french' },
  { value: 'german', label: 'german' },
  { value: 'spanish', label: 'spanish' },
  { value: 'italian', label: 'italian' },
  { value: 'portuguese', label: 'portuguese' },
  { value: 'russian', label: 'russian' },
  { value: 'dutch', label: 'dutch' },
  { value: 'norwegian', label: 'norwegian' },
  { value: 'swedish', label: 'swedish' },
  { value: 'danish', label: 'danish' },
  { value: 'finnish', label: 'finnish' },
  { value: 'hungarian', label: 'hungarian' },
  { value: 'turkish', label: 'turkish' },
]

interface FormValues {
  fts_enabled: boolean
  fts_dictionary: string
  fts_rrf_k: number
  fts_candidate_multiplier: number
  fts_min_rank: number
}

interface PendingDictionarySwap {
  values: FormValues
  newDictionary: string
}

/**
 * Full-text-search admin tuning. Engine knob = `fts_dictionary`
 * (changing it rewrites the GENERATED tsvector column — explicit
 * rebuild flow); retrieval knobs = `fts_enabled`, `fts_rrf_k`,
 * `fts_candidate_multiplier`, `fts_min_rank` (saved in-place).
 *
 * Polling for `ftsRebuildStatus` lives in `RebuildStatusSection`;
 * this section only observes it via the store.
 */
export function FullTextSearchSection() {
  const canRead = usePermission(READ_PERM) || usePermission(MANAGE_PERM)
  const canManage = usePermission(MANAGE_PERM)
  const {
    settings,
    saving,
    error,
    ftsRebuildStatus,
    triggeringFtsRebuild,
  } = Stores.MemoryAdmin
  const form = useForm<FormValues>()
  const [pendingDictionary, setPendingDictionary] =
    useState<PendingDictionarySwap | null>(null)

  // Re-seed the form ONLY when the loaded settings change AND no field
  // is currently dirty. Without the touched-gate the polling reload
  // after a rebuild would clobber pending edits the admin made while
  // waiting (e.g. tweaking fts_rrf_k during the rebuild).
  const lastSettingsRef = useRef<typeof settings>(null)
  useEffect(() => {
    if (!settings) return
    if (lastSettingsRef.current === settings) return
    lastSettingsRef.current = settings
    if (!form.formState.isDirty) {
      form.reset({
        fts_enabled: settings.fts_enabled,
        fts_dictionary: settings.fts_dictionary,
        fts_rrf_k: settings.fts_rrf_k,
        fts_candidate_multiplier: settings.fts_candidate_multiplier,
        fts_min_rank: settings.fts_min_rank,
      })
    }
  }, [settings, form])

  // Watch the form fields so the "both arms off" banner reacts to
  // in-flight toggles (without waiting for the next save round-trip).
  const watchedFtsEnabled = form.watch('fts_enabled')

  // Surface a success toast on the in_progress → idle transition. The
  // settings refetch is driven by the sync event the rebuild worker
  // emits on commit — we just observe its arrival here.
  const wasInProgress = useRef(false)
  useEffect(() => {
    const now = ftsRebuildStatus?.in_progress ?? false
    if (wasInProgress.current && !now) {
      message.success('Full-text search index rebuilt.')
    }
    wasInProgress.current = now
  }, [ftsRebuildStatus?.in_progress])

  if (!canRead) {
    return (
      <Card title="Full-text search" data-testid="memory-fts-card">
        <Alert
          tone="warning"
          title="You don't have permission to view memory admin settings."
          data-testid="memory-fts-no-perm-alert"
        />
      </Card>
    )
  }
  if (!settings)
    return (
      <SettingsSectionStatus
        title="Full-text search"
        error={error}
        onRetry={() => Stores.MemoryAdmin.load()}
      />
    )

  const effectiveFtsEnabled = watchedFtsEnabled ?? settings.fts_enabled
  const bothArmsOff =
    settings.embedding_model_id == null && !effectiveFtsEnabled

  const persistRetrievalKnobs = async (values: FormValues) => {
    try {
      await Stores.MemoryAdmin.update({
        fts_enabled: values.fts_enabled,
        fts_rrf_k: values.fts_rrf_k,
        fts_candidate_multiplier: values.fts_candidate_multiplier,
        fts_min_rank: values.fts_min_rank,
      })
      // Re-seed from the just-saved values AND clear dirty state so a
      // later settings refetch (e.g. another admin's change, or the
      // sync-driven reload after a rebuild) can resume re-seeding the
      // form. Without this, `isDirty` latches `true` after the
      // first save and the form stops syncing.
      form.reset(values)
      message.success('Full-text search settings saved.')
    } catch (error) {
      message.error(
        error instanceof Error
          ? error.message
          : 'Failed to save full-text search settings.',
      )
    }
  }

  const handleSubmit = async (values: FormValues) => {
    const dictionaryChanged = values.fts_dictionary !== settings.fts_dictionary

    if (dictionaryChanged) {
      setPendingDictionary({
        values,
        newDictionary: values.fts_dictionary,
      })
      return
    }
    await persistRetrievalKnobs(values)
  }

  const handleRebuildConfirm = async () => {
    if (!pendingDictionary) return
    const { values, newDictionary } = pendingDictionary
    try {
      // Save the non-dictionary knobs first so the rebuild's atomic
      // dictionary swap doesn't race with a later PUT. The rebuild
      // endpoint itself owns the dictionary write.
      const dictionaryDiffersOnly =
        values.fts_enabled === settings.fts_enabled &&
        values.fts_rrf_k === settings.fts_rrf_k &&
        values.fts_candidate_multiplier === settings.fts_candidate_multiplier &&
        values.fts_min_rank === settings.fts_min_rank
      if (!dictionaryDiffersOnly) {
        await Stores.MemoryAdmin.update({
          fts_enabled: values.fts_enabled,
          fts_rrf_k: values.fts_rrf_k,
          fts_candidate_multiplier: values.fts_candidate_multiplier,
          fts_min_rank: values.fts_min_rank,
        })
      }
      await Stores.MemoryAdmin.triggerFtsRebuild(newDictionary)
      // Re-seed from the saved values and clear touched state so future
      // settings refetches (sync-driven reload after rebuild completes,
      // another admin's change) can update the form again. Without this,
      // `isFieldsTouched()` latches `true` after the first dictionary-
      // swap save and the form stops syncing.
      // Re-seed the form with the saved values and clear dirty/touched state
      // (RHF `reset` makes these the new defaults), so future sync-driven
      // refetches can update the form again.
      form.reset(values)
      setPendingDictionary(null)
      message.info(
        'Full-text search rebuild started. New memories created during the rebuild are picked up automatically.',
      )
      // Kick the status endpoint once so RebuildStatusSection picks
      // up the in_progress flip without waiting a poll cycle.
      void Stores.MemoryAdmin.loadFtsRebuildStatus()
    } catch (error) {
      message.error(
        error instanceof Error
          ? error.message
          : 'Failed to start full-text search rebuild.',
      )
    }
  }

  return (
    <>
      <Card
        title="Full-text search"
        data-testid="memory-fts-card"
        footer={canManage ? (
          <SettingsFormActions
            onSave={form.handleSubmit(handleSubmit)}
            onCancel={() => form.reset()}
            saving={saving || triggeringFtsRebuild}
            saveDisabled={ftsRebuildStatus?.in_progress === true}
            saveTestid="memory-fts-save-btn"
            cancelTestid="memory-fts-cancel-btn"
          />
        ) : undefined}
      >
        {bothArmsOff && (
          <Alert
            tone="warning"
            className="!mb-4"
            title="Both recall arms are disabled."
            data-testid="memory-fts-both-off-alert"
            description={
              <span>
                New memories will still be extracted and stored (if
                extraction is enabled), but chat will not retrieve
                them. To pause memory entirely, turn off the
                deployment-wide <strong>Enable memory</strong> toggle
                in the Engine section instead.
              </span>
            }
          />
        )}
        <Form
          name="memory-admin-fts-form"
          form={form}
          layout="horizontal"
          onSubmit={handleSubmit}
          disabled={!canManage}
          data-testid="memory-fts-form"
        >
          <FormField
            name="fts_enabled"
            label="Enable full-text search"
            description="When off, retrieval skips the FTS arm. If no embedding model is configured, retrieval is disabled entirely."
            valuePropName="checked"
          >
            <Switch aria-label="Enable full-text search retrieval" data-testid="memory-fts-enabled-switch" />
          </FormField>

          <FormField
            name="fts_dictionary"
            label="Dictionary"
            description={
              <span>
                Tokenizer + stemmer. <code>simple</code> = language-agnostic,
                no stemming (default; recommended for multilingual stores).{' '}
                <code>english</code> etc. = Porter stemmer for that language
                only. <strong>Changing this rebuilds the FTS index.</strong>
              </span>
            }
          >
            <Select
              data-testid="memory-fts-dictionary-select"
              options={DICTIONARY_OPTIONS}
              className="max-w-[480px]"
              disabled={!canManage || ftsRebuildStatus?.in_progress === true}
            />
          </FormField>

          <FormField
            name="fts_rrf_k"
            label="RRF k"
            description="RRF blending constant for hybrid retrieval. Higher = more egalitarian; lower = lopsided toward each arm's top-ranked. 60 matches the original RRF paper."
          >
            <InputNumber min={1} max={1000} className="w-40" data-testid="memory-fts-rrf-input" />
          </FormField>

          <FormField
            name="fts_candidate_multiplier"
            label="Candidate multiplier"
            description="Hybrid retrieval pulls top-K × this many candidates from each arm before RRF fusion. Higher = more recall, more DB load. Ignored when hybrid is disabled."
          >
            <InputNumber min={1} max={20} className="w-40" data-testid="memory-fts-candidate-input" />
          </FormField>

          <FormField
            name="fts_min_rank"
            label="Minimum ts_rank_cd"
            description="ts_rank_cd cutoff. 0 = no filter (default). Increase to drop weak lexical matches."
          >
            <InputNumber min={0} max={1} step={0.05} className="w-40" data-testid="memory-fts-minrank-input" />
          </FormField>

        </Form>
      </Card>

      <Dialog
        data-testid="memory-fts-rebuild-dialog"
        open={pendingDictionary !== null}
        title="Rebuild the full-text search index?"
        onOpenChange={(open) => {
          if (!open && (saving || triggeringFtsRebuild)) return
          if (!open) {
            setPendingDictionary(null)
            // Revert just the dictionary field so cancel returns the user
            // to the loaded value; other in-flight edits are preserved.
            form.setValue('fts_dictionary', settings.fts_dictionary)
          }
        }}
        footer={
          <Flex justify="end" className="gap-2">
            <Button
              variant="outline"
              data-testid="memory-fts-rebuild-cancel-btn"
              disabled={saving || triggeringFtsRebuild}
              onClick={() => {
                // Block the cancel during in-flight rebuild: the server-side
                // dictionary swap is already committing and reverting the
                // form field would make the UI briefly disagree with reality.
                if (saving || triggeringFtsRebuild) return
                setPendingDictionary(null)
                // Revert just the dictionary field so cancel returns the user
                // to the loaded value; other in-flight edits are preserved.
                form.setValue('fts_dictionary', settings.fts_dictionary)
              }}
            >
              Keep current dictionary
            </Button>
            <Button
              loading={saving || triggeringFtsRebuild}
              onClick={handleRebuildConfirm}
              data-testid="memory-fts-rebuild-confirm-btn"
            >
              Rebuild
            </Button>
          </Flex>
        }
      >
        <Paragraph>
          Switching to <code>{pendingDictionary?.newDictionary}</code>{' '}
          rewrites <code>user_memories.content_tsv</code> and can take
          several minutes on large stores.
        </Paragraph>
        <Paragraph type="secondary" className="!mb-0 text-sm">
          New memories created during the rebuild are picked up
          automatically. Retrieval continues to work using the old
          dictionary until the rebuild completes.
        </Paragraph>
      </Dialog>
    </>
  )
}
