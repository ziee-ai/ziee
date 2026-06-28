import { Card, Form, FormField, useForm, Switch, Text, message } from '@/components/ui'
import { useEffect } from 'react'
import { Stores } from '@/core/stores'

export function UserRegistrationSettings() {
  const form = useForm<{ enabled: boolean }>({
    defaultValues: { enabled: false },
  })

  // Users store
  const { userRegistrationEnabled, loadingRegistrationSettings, error } =
    Stores.Users

  // Show errors
  useEffect(() => {
    if (error) {
      message.error(error)
      Stores.Users.clearError()
    }
  }, [error])

  // Update form when registration status changes
  useEffect(() => {
    form.setValue('enabled', userRegistrationEnabled)
  }, [userRegistrationEnabled]) // Removed form from dependencies to prevent infinite rerenders

  const handleToggle = async (newValue: boolean) => {
    try {
      await Stores.Users.updateUserRegistrationSettings(newValue)
      message.success(
        `User registration ${newValue ? 'enabled' : 'disabled'} successfully`,
      )
    } catch (error) {
      console.error('Failed to update registration status:', error)
      // Error is handled by the store
    }
  }

  return (
    <Card title="User Registration">
      <Form form={form} onSubmit={() => {}}>
        <div className="flex justify-between items-center">
          <div>
            <Text strong>Enable User Registration</Text>
            <div>
              <Text type="secondary">
                Allow new users to register for accounts
              </Text>
            </div>
          </div>
          <FormField name="enabled" valuePropName="checked" className="mb-0">
            <Switch
              loading={loadingRegistrationSettings}
              onChange={handleToggle}
            />
          </FormField>
        </div>
      </Form>
    </Card>
  )
}
