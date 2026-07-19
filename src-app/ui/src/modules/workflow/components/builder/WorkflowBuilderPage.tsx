import { useEffect } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import { ArrowLeft, Save } from 'lucide-react'
import { Alert, Button, ErrorState, Input, Result, Spin, Text, message } from '@ziee/kit'
import { Stores } from '@ziee/framework/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { WorkflowBuilderStoreDef } from '../../stores/WorkflowBuilder.store'
import { StepList } from './StepList'
import { StepConfigPanel } from './StepConfigPanel'
import { WorkflowInputsEditor } from './WorkflowInputsEditor'
import { BuilderValidationPanel } from './BuilderValidationPanel'
import { LabeledControl } from './builderFields'

const WORKFLOWS_PATH = '/settings/workflows'

/**
 * ITEM-7 — the workflow builder surface. One component backs both the create
 * route (`/settings/workflows/builder`) and the edit route
 * (`/settings/workflows/:id/edit`); the route param decides the mode. Layout is
 * a master step-list + detail config-panel that stacks (list above panel) on
 * narrow viewports and sits side-by-side on wider ones.
 */
export function WorkflowBuilderPage() {
  const { id } = useParams<{ id: string }>()
  const navigate = useNavigate()
  const store = WorkflowBuilderStoreDef.use()
  const isEdit = !!id

  // Permission gate (A10): route.permission is advisory only (no app permissionGate
  // is registered), so a direct URL to the builder would otherwise render ungated.
  // Create needs workflows::install; edit needs workflows::manage.
  const canAccess = usePermission(
    isEdit ? Permissions.WorkflowsManage : Permissions.WorkflowsInstall,
  )

  // Load an existing definition (edit) or start blank (create) on mount / id change.
  useEffect(() => {
    if (!canAccess) return
    if (id) void store.load(id)
    else store.initEmpty()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id, canAccess])

  const {
    name,
    def,
    dirty,
    saving,
    loading,
    loadError,
    validation,
    workflowId,
    deletedExternally,
  } = store

  const hasErrors = (validation?.errors?.length ?? 0) > 0
  const stepCount = def.steps.length

  // Existing workflow's display name (edit mode) — best-effort from the list.
  const existingName = Stores.Workflow.workflows.find(
    w => w.id === workflowId,
  )?.name

  const saveDisabled =
    saving ||
    hasErrors ||
    stepCount === 0 ||
    (!isEdit && !name.trim())

  const handleSave = async () => {
    try {
      const workflow = await store.save()
      message.success(isEdit ? 'Workflow saved' : 'Workflow created')
      if (!isEdit) {
        // Switch to the edit route so subsequent saves update in place.
        navigate(`${WORKFLOWS_PATH}/${workflow.id}/edit`, { replace: true })
      }
    } catch (e) {
      message.error(e instanceof Error ? e.message : 'Failed to save workflow')
    }
  }

  // A10 content gate: deny direct-URL access when the user lacks the perm.
  // Rendered WITHOUT the builder's SettingsPageContainer, so `wf-builder-page-title`
  // is absent for an unpermitted user (mirrors the settings section-forbidden result).
  if (!canAccess) {
    return (
      <Result
        data-testid="settings-forbidden-result"
        status="403"
        title="Not authorized"
        subtitle="You don't have permission to author workflows."
      />
    )
  }

  return (
    <SettingsPageContainer
      data-testid="wf-builder-page-title"
      title={isEdit ? 'Edit workflow' : 'New workflow'}
      subtitle="Compose the steps your assistant runs, in order"
    >
      <div className="flex flex-col gap-4">
        {deletedExternally && (
          <Alert
            data-testid="wf-builder-deleted-alert"
            tone="error"
            title="This workflow was deleted"
            description="It was removed on another device. Your unsaved changes are still here — save to recreate it, or go back."
          />
        )}

        {loadError ? (
          <ErrorState
            resource="workflow"
            description="Couldn't load this workflow's definition."
            details={loadError}
            onRetry={() => id && store.load(id)}
            data-testid="wf-builder-load-error"
          />
        ) : loading ? (
          <div className="flex justify-center py-12">
            <Spin label="Loading workflow" />
          </div>
        ) : (
          <>
            {/* Action bar */}
            <div className="flex items-center gap-3 flex-wrap">
              <Button
                type="button"
                variant="ghost"
                icon={<ArrowLeft />}
                data-testid="wf-builder-back-btn"
                onClick={() => navigate(WORKFLOWS_PATH)}
              >
                Back
              </Button>
              <div className="flex-1 min-w-0">
                {isEdit ? (
                  existingName && (
                    <Text strong className="truncate">
                      {existingName}
                    </Text>
                  )
                ) : (
                  <div className="max-w-md">
                    <LabeledControl label="Workflow name" required>
                      <Input
                        data-testid="wf-builder-name"
                        value={name}
                        onChange={e => store.setName(e.target.value)}
                        placeholder="e.g. Literature triage"
                      />
                    </LabeledControl>
                  </div>
                )}
              </div>
              {dirty && (
                <Text type="secondary" className="text-xs">
                  Unsaved changes
                </Text>
              )}
              <Button
                type="button"
                variant="default"
                icon={<Save />}
                loading={saving}
                disabled={saveDisabled}
                data-testid="wf-builder-save-btn"
                onClick={handleSave}
              >
                Save
              </Button>
            </div>

            <WorkflowInputsEditor store={store} />

            {/* Master (step list) / detail (config panel). Stacks on narrow
                viewports, side-by-side from `md` up. */}
            <div className="flex flex-col md:flex-row gap-4">
              <div className="md:w-80 shrink-0">
                <StepList store={store} />
              </div>
              <div className="flex-1 min-w-0">
                <StepConfigPanel store={store} />
              </div>
            </div>

            <BuilderValidationPanel store={store} />
          </>
        )}
      </div>
    </SettingsPageContainer>
  )
}
