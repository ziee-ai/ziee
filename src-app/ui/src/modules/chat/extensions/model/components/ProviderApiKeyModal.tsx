import { useState } from 'react'
import { Modal, Form, Input, Typography, Alert } from 'antd'
import { Stores } from '@/core/stores'

interface ProviderApiKeyModalProps {
  providerId: string
  providerName: string
  modelId: string
  onSuccess: (modelId: string) => void
  onCancel: () => void
}

/**
 * ProviderApiKeyModal
 * Shown inline inside ModelSelector when user selects a model whose
 * provider has no API key configured. Lets the user save their own key
 * before the model is selected.
 */
export function ProviderApiKeyModal({
  providerId,
  providerName,
  modelId,
  onSuccess,
  onCancel,
}: ProviderApiKeyModalProps) {
  const [apiKey, setApiKey] = useState('')
  const [error, setError] = useState<string | null>(null)
  const { saving } = Stores.UserProviderKeys

  const handleOk = async () => {
    const trimmed = apiKey.trim()
    if (!trimmed) {
      setError('API key cannot be empty')
      return
    }
    setError(null)
    try {
      await Stores.UserProviderKeys.saveKey(providerId, trimmed)
      onSuccess(modelId)
    } catch (err: any) {
      setError(err.message || 'Failed to save API key')
    }
  }

  return (
    <Modal
      open
      title={`API Key Required — ${providerName}`}
      onOk={handleOk}
      onCancel={onCancel}
      okText="Save & Select Model"
      cancelText="Cancel"
      confirmLoading={saving}
      destroyOnClose
    >
      <Typography.Paragraph type="secondary">
        This provider doesn&apos;t have a system API key configured. Enter your
        own API key to use models from <strong>{providerName}</strong>.
      </Typography.Paragraph>
      <Form layout="vertical">
        <Form.Item label="API Key">
          <Input.Password
            value={apiKey}
            onChange={e => setApiKey(e.target.value)}
            placeholder="sk-..."
            autoFocus
            onPressEnter={handleOk}
          />
        </Form.Item>
        {error && <Alert type="error" message={error} showIcon />}
      </Form>
      <Typography.Text type="secondary" className="text-xs">
        Your key is stored securely and only used for inference. You can manage
        keys in{' '}
        <Typography.Link href="/settings/user-llm-providers">
          Settings → LLM Providers
        </Typography.Link>
        .
      </Typography.Text>
    </Modal>
  )
}
