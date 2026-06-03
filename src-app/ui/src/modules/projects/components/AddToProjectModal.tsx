import { useEffect, useMemo, useState } from 'react'
import { App, Modal, Select, Typography } from 'antd'
import { Stores } from '@/core/stores'

const { Text } = Typography

interface AddToProjectModalProps {
  open: boolean
  conversationId: string | null
  onClose: () => void
  onAttached?: (projectId: string) => void
}

/**
 * Project-picker modal. Reads the projects list from
 * `Stores.Projects` (loaded on mount if not already); user picks a
 * project; on confirm, calls `Stores.Projects.attachConversation`.
 */
export function AddToProjectModal({
  open,
  conversationId,
  onClose,
  onAttached,
}: AddToProjectModalProps) {
  const { message } = App.useApp()
  const { projects, isInitialized, loading } = Stores.Projects
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [submitting, setSubmitting] = useState(false)

  useEffect(() => {
    if (open && !isInitialized && !loading) {
      void Stores.Projects.loadProjects()
    }
  }, [open, isInitialized, loading])

  // Reset selection when the modal closes so the next open starts fresh.
  useEffect(() => {
    if (!open) {
      setSelectedId(null)
      setSubmitting(false)
    }
  }, [open])

  const options = useMemo(
    () =>
      Array.from(projects.values()).map(p => ({
        value: p.id,
        label: p.name,
      })),
    [projects],
  )

  const handleOk = async () => {
    if (!selectedId || !conversationId) return
    setSubmitting(true)
    try {
      await Stores.Projects.attachConversation(selectedId, conversationId)
      message.success('Added to project')
      onAttached?.(selectedId)
      onClose()
    } catch (err) {
      message.error(
        err instanceof Error ? err.message : 'Failed to add to project',
      )
      setSubmitting(false)
    }
  }

  return (
    <Modal
      title="Add to project"
      open={open}
      onCancel={onClose}
      onOk={handleOk}
      okText="Add"
      okButtonProps={{ disabled: !selectedId, loading: submitting }}
      destroyOnHidden
    >
      <div className="flex flex-col gap-3">
        <Text type="secondary">
          Pick a project to file this conversation into. It will start
          receiving the project's instructions, knowledge files, and
          MCP defaults on subsequent sends.
        </Text>
        <Select
          placeholder="Pick a project…"
          showSearch={{ optionFilterProp: 'label' }}
          loading={loading}
          options={options}
          value={selectedId ?? undefined}
          onChange={v => setSelectedId(v ?? null)}
          notFoundContent={
            loading ? 'Loading…' : 'No projects — create one first.'
          }
          className="w-full"
        />
      </div>
    </Modal>
  )
}
