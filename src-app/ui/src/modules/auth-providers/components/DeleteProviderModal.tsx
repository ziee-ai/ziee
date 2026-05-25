import { useState } from 'react'
import { Alert, Modal, Typography } from 'antd'
import { Stores } from '@/core/stores'
import type { AuthProviderResponse } from '@/api-client/types'

const { Paragraph, Text } = Typography

interface Props {
  open: boolean
  provider: AuthProviderResponse | null
  onClose: () => void
}

/**
 * Delete-provider confirmation modal. We don't know affected_user_links
 * until the user confirms (it's in the delete response), so the warning
 * is generic; the response message after delete carries the exact count.
 */
export function DeleteProviderModal({ open, provider, onClose }: Props) {
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  if (!provider) return null

  const onConfirm = async () => {
    setLoading(true)
    setError(null)
    try {
      await Stores.AuthProvidersAdmin.deleteProvider(provider.id)
      setLoading(false)
      onClose()
    } catch (e: any) {
      setError(e?.message ?? 'Failed to delete provider')
      setLoading(false)
    }
  }

  return (
    <Modal
      open={open}
      onCancel={onClose}
      title="Delete auth provider"
      onOk={onConfirm}
      okText="Delete"
      okButtonProps={{ danger: true, loading }}
      cancelButtonProps={{ disabled: loading }}
    >
      <Paragraph>
        Delete <Text strong>{provider.name}</Text>?
      </Paragraph>
      <Alert
        type="warning"
        showIcon
        message="Users linked through this provider will lose this sign-in method. Their accounts remain — they must sign in with another method (e.g. password) and re-link if needed."
        className="mb-3"
      />
      {error && <Alert type="error" message={error} showIcon />}
    </Modal>
  )
}
