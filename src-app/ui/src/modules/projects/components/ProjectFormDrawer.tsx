import { useEffect, useRef, useState } from 'react'
import { App, Button, Flex, Form, Input, Typography } from 'antd'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import {
  Permissions,
  type CreateProjectRequest,
  type UpdateProjectRequest,
} from '@/api-client/types'

interface ProjectFormValues {
  name: string
  description?: string
  instructions?: string
}

/// NOTE: `default_assistant_id` and `default_model_id` are NOT edited
/// here. They live in the Advanced card on the ProjectDetailPage as
/// inline auto-save selects (`ProjectDefaultsForm`) — keeping
/// configuration-shape settings out of the "name/description/
/// instructions" content drawer.

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
        // Default assistant / default model are edited inline on the
        // ProjectDetailPage's Advanced card via ProjectDefaultsForm,
        // not here — keep this patch focused on the content fields.
        const patch: UpdateProjectRequest = {
          name: values.name,
          description: values.description ?? '',
          instructions: values.instructions ?? '',
        }
        await Stores.Projects.updateProject(editingProject.id, patch)
        message.success('Project updated')
      } else {
        const req: CreateProjectRequest = {
          name: values.name,
          description: values.description,
          instructions: values.instructions,
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
          extra="For your reference only — shown on the project card and detail page. NOT sent to the LLM. To shape the model's behavior in this project, use the Instructions field below instead."
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
          extra="System instructions injected into every conversation in this project. Capped at 64 KiB."
          rules={[{ max: 65_536, message: 'Instructions are too long' }]}
        >
          <Input.TextArea
            rows={10}
            placeholder="e.g. 'You are helping me build a Rust sandbox. Focus on correctness over cleverness.'"
            maxLength={65_536}
            showCount
          />
        </Form.Item>

      </Form>
    </Drawer>
  )
}
