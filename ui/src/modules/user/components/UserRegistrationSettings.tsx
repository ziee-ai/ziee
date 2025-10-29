import { App, Card, Form, Switch, Typography } from 'antd'
import { useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { Stores } from '@/core/stores'
import {
  clearUsersStoreError,
  loadUserRegistrationSettings,
  updateUserRegistrationSettings,
} from '../store'

const { Text } = Typography

export function UserRegistrationSettings() {
  const { t } = useTranslation()
  const { message } = App.useApp()
  const [form] = Form.useForm()

  // Users store
  const { userRegistrationEnabled, loadingRegistrationSettings, error } =
    Stores.Users

  useEffect(() => {
    loadUserRegistrationSettings()
  }, [])

  // Show errors
  useEffect(() => {
    if (error) {
      message.error(error)
      clearUsersStoreError()
    }
  }, [error, message])

  // Update form when registration status changes
  useEffect(() => {
    form.setFieldsValue({ enabled: userRegistrationEnabled })
  }, [userRegistrationEnabled]) // Removed form from dependencies to prevent infinite rerenders

  const handleFormChange = async (changedValues: any) => {
    if ('enabled' in changedValues) {
      const newValue = changedValues.enabled

      try {
        await updateUserRegistrationSettings(newValue)
        message.success(
          `User registration ${newValue ? 'enabled' : 'disabled'} successfully`,
        )
      } catch (error) {
        console.error('Failed to update registration status:', error)
        // Error is handled by the store
      }
    }
  }

  return (
    <Card title={t('admin.userRegistration')}>
      <Form
        form={form}
        onValuesChange={handleFormChange}
        initialValues={{ enabled: userRegistrationEnabled }}
      >
        <div className="flex justify-between items-center">
          <div>
            <Text strong>Enable User Registration</Text>
            <div>
              <Text type="secondary">
                Allow new users to register for accounts
              </Text>
            </div>
          </div>
          <Form.Item name="enabled" valuePropName="checked" className="mb-0">
            <Switch loading={loadingRegistrationSettings} size="default" />
          </Form.Item>
        </div>
      </Form>
    </Card>
  )
}
