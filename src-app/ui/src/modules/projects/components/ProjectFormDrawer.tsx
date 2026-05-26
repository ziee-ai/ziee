import { useCallback, useEffect, useRef, useState } from 'react'
import { App, Button, Flex, Form, Input, Select, Typography } from 'antd'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { ApiClient } from '@/api-client'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import {
  Permissions,
  type Assistant,
  type CreateProjectRequest,
  type LlmModel,
  type UpdateProjectRequest,
} from '@/api-client/types'

interface ProjectFormValues {
  name: string
  description?: string
  instructions?: string
  default_assistant_id?: string | null
  default_model_id?: string | null
}

export function ProjectFormDrawer() {
  const { message } = App.useApp()
  const { open, editingProject, loading } = Stores.ProjectDrawer
  const [form] = Form.useForm<ProjectFormValues>()

  // Permission gating (audit Q2). `canSave` is the permission required
  // for THIS drawer mode: ProjectsEdit when editing, ProjectsCreate
  // when creating. When false, Form is `disabled`, Submit is HIDDEN
  // (not just disabled), and Cancel becomes "Close".
  const canEdit = usePermission(Permissions.ProjectsEdit)
  const canCreate = usePermission(Permissions.ProjectsCreate)
  const isEdit = !!editingProject
  const canSave = isEdit ? canEdit : canCreate

  // Local cache of the default-asset pickers' option lists.
  const [assistants, setAssistants] = useState<Assistant[]>([])
  const [models, setModels] = useState<LlmModel[]>([])
  const [optionsLoading, setOptionsLoading] = useState(false)

  /// Mounted/open flag — closes audit N10. Late-landing fetches from
  /// a closed drawer must NOT setState, or React warns + we leak.
  const mountedRef = useRef(true)
  useEffect(() => {
    mountedRef.current = true
    return () => {
      mountedRef.current = false
    }
  }, [])

  /// Tracks whether the drawer was just opened "fresh" (vs an in-place
  /// update via event). Closes audit N6: if `editingProject` changes
  /// while the user has unsaved edits (e.g., another tab updated the
  /// project), the form must NOT overwrite their work.
  const lastOpenedSubjectId = useRef<string | null>(null)
  const [remoteUpdatedWhileEditing, setRemoteUpdatedWhileEditing] =
    useState(false)

  // Lazy-load the picker options the first time the drawer opens.
  // Wrapped in useCallback so the event-subscription effect below can
  // call the same refetch routine without re-binding listeners.
  const refetchOptions = useCallback(async () => {
    if (!mountedRef.current) return
    setOptionsLoading(true)
    try {
      const [assistantsResp, modelsResp] = await Promise.all([
        ApiClient.Assistant.list({ page: 1, limit: 100 }),
        ApiClient.LlmModel.list({ page: 1, perPage: 100 }),
      ])
      if (!mountedRef.current) return
      setAssistants(assistantsResp.assistants ?? [])
      setModels(modelsResp.models ?? [])
    } catch (err) {
      // Non-fatal — user can still save without picking a default.
      console.warn('Failed to load default-asset options', err)
    } finally {
      if (mountedRef.current) setOptionsLoading(false)
    }
  }, [])

  useEffect(() => {
    if (!open) return
    void refetchOptions()
  }, [open, refetchOptions])

  // While the drawer is open, watch for asset lifecycle changes in
  // OTHER tabs / components and refresh so a newly-created assistant
  // (or freshly enabled model) appears in the pickers without needing
  // to close + reopen. Closes audit F3.
  useEffect(() => {
    if (!open) return
    const GROUP = 'ProjectFormDrawer'
    const eventBus = Stores.EventBus

    const off1 = eventBus.on('assistant.created', () => void refetchOptions(), GROUP)
    const off2 = eventBus.on('assistant.deleted', () => void refetchOptions(), GROUP)
    const off3 = eventBus.on('assistant.updated', () => void refetchOptions(), GROUP)
    const off4 = eventBus.on('llm_model.enabled', () => void refetchOptions(), GROUP)
    const off5 = eventBus.on('llm_model.disabled', () => void refetchOptions(), GROUP)
    const off6 = eventBus.on('llm_model.deleted', () => void refetchOptions(), GROUP)

    return () => {
      off1()
      off2()
      off3()
      off4()
      off5()
      off6()
    }
  }, [open, refetchOptions])

  // Reset form on a FRESH open (subject change), preserve user edits
  // on in-place updates (audit N6). The reset only fires when:
  //   - The drawer just opened, OR
  //   - The editingProject id changed (user switched targets)
  // Form values from a remote `project.updated` event are NOT applied
  // if the user has touched the form — we just show a banner.
  useEffect(() => {
    if (!open) {
      lastOpenedSubjectId.current = null
      setRemoteUpdatedWhileEditing(false)
      form.resetFields()
      return
    }
    const subject = editingProject?.id ?? '__create__'
    if (lastOpenedSubjectId.current !== subject) {
      // Fresh subject — safe to reset.
      lastOpenedSubjectId.current = subject
      setRemoteUpdatedWhileEditing(false)
      form.setFieldsValue({
        name: editingProject?.name ?? '',
        description: editingProject?.description ?? '',
        instructions: editingProject?.instructions ?? '',
        default_assistant_id: editingProject?.default_assistant_id ?? null,
        default_model_id: editingProject?.default_model_id ?? null,
      })
    } else {
      // Same subject, but `editingProject` changed (likely from a
      // `project.updated` event). If the user has UNSAVED edits, show
      // a "remote changes available" banner instead of clobbering.
      if (form.isFieldsTouched()) {
        setRemoteUpdatedWhileEditing(true)
      } else {
        // No user edits — silently take the remote values.
        form.setFieldsValue({
          name: editingProject?.name ?? '',
          description: editingProject?.description ?? '',
          instructions: editingProject?.instructions ?? '',
          default_assistant_id: editingProject?.default_assistant_id ?? null,
          default_model_id: editingProject?.default_model_id ?? null,
        })
      }
    }
  }, [open, editingProject, form])

  const handleDiscardLocalEdits = () => {
    setRemoteUpdatedWhileEditing(false)
    form.setFieldsValue({
      name: editingProject?.name ?? '',
      description: editingProject?.description ?? '',
      instructions: editingProject?.instructions ?? '',
      default_assistant_id: editingProject?.default_assistant_id ?? null,
      default_model_id: editingProject?.default_model_id ?? null,
    })
  }

  const handleClose = () => {
    if (loading) return
    Stores.ProjectDrawer.closeProjectDrawer()
  }

  const handleSubmit = async (values: ProjectFormValues) => {
    Stores.ProjectDrawer.setProjectDrawerLoading(true)
    try {
      if (isEdit && editingProject) {
        // Tri-state on default_*_id (backend Option<Option<Uuid>>):
        //   undefined → field omitted → "no change"
        //   null      → JSON `null`   → "clear"
        //   string    → uuid          → "set"
        // The generated TS types flatten the outer Option to
        // `string | undefined`, so we cast through `unknown` to allow
        // wiring null through to the server when the user clears a
        // picker. The backend's `deserialize_nullable_field` reads it
        // correctly. Tracked as a codegen improvement.
        const patch: UpdateProjectRequest = {
          name: values.name,
          description: values.description ?? '',
          instructions: values.instructions ?? '',
          default_assistant_id: (values.default_assistant_id ??
            null) as unknown as string | undefined,
          default_model_id: (values.default_model_id ??
            null) as unknown as string | undefined,
        }
        await Stores.Projects.updateProject(editingProject.id, patch)
        message.success('Project updated')
      } else {
        const req: CreateProjectRequest = {
          name: values.name,
          description: values.description,
          instructions: values.instructions,
          default_assistant_id: values.default_assistant_id ?? undefined,
          default_model_id: values.default_model_id ?? undefined,
        }
        await Stores.Projects.createProject(req)
        message.success('Project created')
      }
      Stores.ProjectDrawer.closeProjectDrawer()
    } catch (err) {
      message.error(
        err instanceof Error ? err.message : 'Failed to save project',
      )
    } finally {
      Stores.ProjectDrawer.setProjectDrawerLoading(false)
    }
  }

  return (
    <Drawer
      title={isEdit ? 'Edit Project' : 'New Project'}
      open={open}
      onClose={handleClose}
      size={600}
      destroyOnHidden
      footer={
        // Cancel-before-Submit, right-aligned via Flex per
        // ui-consistency-patterns.md. Cancel→Close label switches on
        // canSave; Submit is GATED (not just disabled) so it doesn't
        // appear at all in read-only mode.
        <Flex className="justify-end gap-2">
          <Button onClick={handleClose} disabled={loading}>
            {canSave ? 'Cancel' : 'Close'}
          </Button>
          {canSave && (
            <Button
              type="primary"
              htmlType="submit"
              onClick={() => form.submit()}
              loading={loading}
            >
              {isEdit ? 'Save' : 'Create'}
            </Button>
          )}
        </Flex>
      }
    >
      <Form<ProjectFormValues>
        form={form}
        layout="vertical"
        disabled={!canSave}
        onFinish={handleSubmit}
      >
        {remoteUpdatedWhileEditing && (
          <div className="mb-3 p-2 rounded border border-orange-300 bg-orange-50">
            <Typography.Text type="warning" className="text-sm">
              Remote changes detected while you were editing. Your local edits
              are preserved.{' '}
            </Typography.Text>
            <Button
              type="link"
              size="small"
              onClick={handleDiscardLocalEdits}
              className="!p-0"
            >
              Discard my edits + load remote
            </Button>
          </div>
        )}
        <Form.Item
          name="name"
          label="Name"
          rules={[
            { required: true, message: 'Name is required' },
            { max: 255, message: 'Name must be at most 255 characters' },
          ]}
        >
          <Input placeholder="My project" autoFocus />
        </Form.Item>

        <Form.Item
          name="description"
          label="Description"
          rules={[{ max: 4096, message: 'Description is too long' }]}
        >
          <Input.TextArea
            rows={3}
            placeholder="Optional short description"
            maxLength={4096}
          />
        </Form.Item>

        <Form.Item
          name="instructions"
          label="Instructions"
          tooltip="System instructions injected into every conversation in this project. Capped at 64 KiB."
          rules={[{ max: 65_536, message: 'Instructions are too long' }]}
        >
          <Input.TextArea
            rows={10}
            placeholder="e.g. 'You are helping me build a Rust sandbox. Focus on correctness over cleverness.'"
            maxLength={65_536}
            showCount
          />
        </Form.Item>

        <Form.Item
          name="default_assistant_id"
          label="Default assistant"
          tooltip="Pre-selected when creating a new conversation in this project. Users can override per-conversation."
        >
          <Select
            allowClear
            placeholder="No default"
            loading={optionsLoading}
            options={(() => {
              const opts = assistants.map(a => ({
                value: a.id as string,
                label: a.name,
              }))
              const current = editingProject?.default_assistant_id
              if (current && !opts.some(o => o.value === current)) {
                opts.unshift({ value: current, label: '(deleted)' })
              }
              return opts
            })()}
            showSearch={{ optionFilterProp: 'label' }}
          />
        </Form.Item>

        <Form.Item
          name="default_model_id"
          label="Default model"
          tooltip="Snapshotted onto each conversation created in this project (when no explicit model is selected)."
        >
          <Select
            allowClear
            placeholder="No default"
            loading={optionsLoading}
            options={(() => {
              const opts = models.map(m => ({
                value: m.id as string,
                label: m.display_name || m.name,
              }))
              const current = editingProject?.default_model_id
              if (current && !opts.some(o => o.value === current)) {
                opts.unshift({ value: current, label: '(deleted)' })
              }
              return opts
            })()}
            showSearch={{ optionFilterProp: 'label' }}
          />
        </Form.Item>
      </Form>
    </Drawer>
  )
}
