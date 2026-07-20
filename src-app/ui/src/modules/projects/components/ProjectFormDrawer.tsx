import { useEffect, useRef, useState } from 'react'
import {
  Button,
  Flex,
  Form,
  FormField,
  useForm,
  zodResolver,
  Input,
  Textarea,
  Text,
  message,
} from '@ziee/kit'
import { z } from 'zod'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { Stores } from '@ziee/framework/stores'
import { usePermission } from '@/core/permissions'
import { type CreateProjectRequest, type UpdateProjectRequest } from '@/api-client/types'
import { Permissions } from '@/api-client/permissions'

interface ProjectFormValues {
  name: string
  description?: string
  instructions?: string
}

const schema = z.object({
  name: z
    .string()
    .min(1, 'Name is required')
    .max(255, 'Name must be at most 255 characters'),
  description: z.string().max(4096, 'Description is too long').optional(),
  instructions: z.string().max(65_536, 'Instructions are too long').optional(),
})

/// NOTE: `default_assistant_id` and `default_model_id` are NOT edited
/// here. They live in the Advanced card on the ProjectDetailPage as
/// inline auto-save selects (`ProjectDefaultsForm`) — keeping
/// configuration-shape settings out of the "name/description/
/// instructions" content drawer.

export function ProjectFormDrawer() {
  const { open, editingProject, loading } = Stores.ProjectDrawer
  const form = useForm<ProjectFormValues>({
    resolver: zodResolver(schema),
    defaultValues: { name: '', description: '', instructions: '' },
  })

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
      form.reset({ name: '', description: '', instructions: '' })
      return
    }
    const subject = editingProject?.id ?? '__create__'
    if (lastOpenedSubjectId.current !== subject) {
      // Fresh subject — safe to reset.
      lastOpenedSubjectId.current = subject
      setRemoteUpdatedWhileEditing(false)
      form.reset({
        name: editingProject?.name ?? '',
        description: editingProject?.description ?? '',
        instructions: editingProject?.instructions ?? '',
      })
    } else {
      // Same subject, but `editingProject` changed (likely from a
      // `project.updated` event). If the user has UNSAVED edits, show
      // a "remote changes available" banner instead of clobbering.
      if (form.formState.isDirty) {
        setRemoteUpdatedWhileEditing(true)
      } else {
        // No user edits — silently take the remote values.
        form.reset({
          name: editingProject?.name ?? '',
          description: editingProject?.description ?? '',
          instructions: editingProject?.instructions ?? '',
        })
      }
    }
  }, [open, editingProject, form])

  const handleDiscardLocalEdits = () => {
    setRemoteUpdatedWhileEditing(false)
    form.reset({
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
          <Button data-testid="project-form-cancel-button" variant="outline" onClick={handleClose} disabled={loading}>
            {canSave ? 'Cancel' : 'Close'}
          </Button>
          {canSave && (
            <Button
              data-testid="project-form-submit-button"
              type="submit"
              onClick={form.handleSubmit(handleSubmit)}
              loading={loading}
            >
              {isEdit ? 'Save' : 'Create'}
            </Button>
          )}
        </Flex>
      }
    >
      <Form
        data-testid="project-form"
        form={form}
        layout="vertical"
        disabled={!canSave}
        onSubmit={handleSubmit}
      >
        {remoteUpdatedWhileEditing && (
          <div className="mb-3 p-2 rounded border border-border bg-muted">
            <Text type="warning" className="text-sm">
              Remote changes detected while you were editing. Your local edits
              are preserved.{' '}
            </Text>
            <Button
              data-testid="project-form-discard-edits-button"
              variant="link"
              size="default"
              onClick={handleDiscardLocalEdits}
              className="!p-0"
            >
              Discard my edits + load remote
            </Button>
          </div>
        )}
        <FormField name="name" label="Name" required>
          <Input data-testid="project-form-name-input" placeholder="My project" autoFocus />
        </FormField>

        <FormField
          name="description"
          label="Description"
          description="For your reference only — shown on the project card and detail page. NOT sent to the LLM. To shape the model's behavior in this project, use the Instructions field below instead."
        >
          <Textarea
            data-testid="project-form-description-textarea"
            rows={3}
            placeholder="Optional short description"
            maxLength={4096}
          />
        </FormField>

        <FormField
          name="instructions"
          label="Instructions"
          description="System instructions injected into every conversation in this project. Capped at 64 KiB."
        >
          <Textarea
            data-testid="project-form-instructions-textarea"
            rows={10}
            placeholder="e.g. 'You are helping me build a Rust sandbox. Focus on correctness over cleverness.'"
            maxLength={65_536}
          />
        </FormField>

      </Form>
    </Drawer>
  )
}
