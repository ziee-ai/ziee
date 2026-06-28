import { useEffect } from 'react'
import {
  Alert,
  Avatar,
  Button,
  Card,
  Descriptions,
  Divider,
  Flex,
  Form,
  Input,
  Tag,
  Typography,
  message,
} from 'antd'
import { UserOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'

const { Text } = Typography

interface ProfileFormValues {
  display_name: string
  username: string
}

interface PasswordFormValues {
  current_password: string
  new_password: string
  confirm_password: string
}

export function ProfileSettingsPage() {
  // Read ALL store fields at the top, before any early return (hooks rule).
  const { user, hasPassword } = Stores.Auth
  const { savingProfile, savingPassword } = Stores.Profile
  const canEdit = usePermission(Permissions.ProfileEdit)
  const [profileForm] = Form.useForm<ProfileFormValues>()
  const [passwordForm] = Form.useForm<PasswordFormValues>()

  // Refresh /me on mount so `hasPassword` + profile fields are accurate
  // even when the user arrived via an in-session login (authenticateUser
  // sets `user` from the login response, which carries no `has_password`).
  useEffect(() => {
    void Stores.Auth.refreshCurrentUser()
  }, [])

  useEffect(() => {
    if (user) {
      profileForm.setFieldsValue({
        display_name: user.display_name ?? '',
        username: user.username,
      })
    }
  }, [user, profileForm])

  if (!user) return null

  const handleProfileSubmit = async (values: ProfileFormValues) => {
    try {
      await Stores.Profile.updateProfile({
        username: values.username.trim(),
        display_name: values.display_name.trim(),
      })
      message.success('Profile saved.')
    } catch (error) {
      message.error(
        error instanceof Error ? error.message : 'Failed to save profile.',
      )
    }
  }

  const handlePasswordSubmit = async (values: PasswordFormValues) => {
    try {
      await Stores.Profile.changePassword({
        current_password: values.current_password,
        new_password: values.new_password,
      })
      message.success('Password changed.')
      passwordForm.resetFields()
    } catch (error) {
      message.error(
        error instanceof Error ? error.message : 'Failed to change password.',
      )
    }
  }

  return (
    <SettingsPageContainer title="Profile">
      <Card title="Account">
        {/* Wrap the body in a flex column with explicit gap. Per-child
            mb-* wasn't taking effect — antd v6's Card body layout
            collapses sibling margins; flex gap is the reliable lever. */}
        <Flex vertical gap={24}>
        <Flex align="center" gap={16}>
          <Avatar
            size={64}
            src={user.avatar_url || undefined}
            icon={<UserOutlined />}
          />
          <Flex gap={8} wrap="wrap">
            <Tag color={user.is_admin ? 'gold' : 'default'}>
              {user.is_admin ? 'Administrator' : 'User'}
            </Tag>
            <Tag color={user.email_verified ? 'green' : 'orange'}>
              {user.email_verified ? 'Email verified' : 'Email unverified'}
            </Tag>
          </Flex>
        </Flex>

        <Descriptions
          size="small"
          column={{ xs: 1, sm: 2 }}
          colon={false}
        >
          <Descriptions.Item label="Email">{user.email}</Descriptions.Item>
          <Descriptions.Item label="Member since">
            {new Date(user.created_at).toLocaleDateString()}
          </Descriptions.Item>
          <Descriptions.Item label="Last login">
            {user.last_login_at
              ? new Date(user.last_login_at).toLocaleDateString()
              : 'Never'}
          </Descriptions.Item>
        </Descriptions>

        {!canEdit && (
          <Alert
            type="info"
            showIcon
            message="You don't have permission to edit your profile. Fields are read-only."
            className="mb-3"
          />
        )}
        <Form
          name="profile-form"
          form={profileForm}
          layout="horizontal"
          labelCol={{ flex: '160px' }}
          wrapperCol={{ flex: 'auto' }}
          labelAlign="left"
          colon={false}
          onFinish={handleProfileSubmit}
          disabled={!canEdit}
        >
          <Form.Item
            name="display_name"
            label="Display name"
            extra="The name shown to others. Optional."
          >
            <Input maxLength={255} placeholder="Your display name" />
          </Form.Item>
          <Form.Item
            name="username"
            label="Username"
            rules={[
              { required: true, message: 'Username is required' },
              { whitespace: true, message: 'Username cannot be blank' },
            ]}
          >
            <Input maxLength={255} placeholder="Your username" />
          </Form.Item>

          {canEdit && (
            <>
              <Divider className="!my-3" />
              <Flex justify="end" gap={8}>
                <Button
                  htmlType="button"
                  disabled={savingProfile}
                  onClick={() => {
                    // Discard unsaved edits — restore the persisted values.
                    profileForm.setFieldsValue({
                      display_name: user.display_name ?? '',
                      username: user.username,
                    })
                  }}
                >
                  Cancel
                </Button>
                <Button
                  type="primary"
                  htmlType="submit"
                  loading={savingProfile}
                >
                  Save
                </Button>
              </Flex>
            </>
          )}
        </Form>
        </Flex>
      </Card>

      {canEdit && (
        <Card title="Password">
          {hasPassword ? (
            <Form
              name="password-form"
              form={passwordForm}
              layout="horizontal"
              labelCol={{ flex: '160px' }}
              wrapperCol={{ flex: 'auto' }}
              labelAlign="left"
              colon={false}
              onFinish={handlePasswordSubmit}
            >
              <Form.Item
                name="current_password"
                label="Current password"
                rules={[
                  { required: true, message: 'Enter your current password' },
                ]}
              >
                <Input.Password
                  autoComplete="current-password"
                  placeholder="Current password"
                />
              </Form.Item>
              <Form.Item
                name="new_password"
                label="New password"
                rules={[
                  { required: true, message: 'Enter a new password' },
                  { min: 8, message: 'Password must be at least 8 characters' },
                  {
                    max: 72,
                    message: 'Password must be at most 72 characters',
                  },
                ]}
              >
                <Input.Password
                  autoComplete="new-password"
                  placeholder="New password"
                  maxLength={72}
                />
              </Form.Item>
              <Form.Item
                name="confirm_password"
                label="Confirm new password"
                dependencies={['new_password']}
                rules={[
                  { required: true, message: 'Re-enter the new password' },
                  ({ getFieldValue }) => ({
                    validator(_, value) {
                      if (!value || getFieldValue('new_password') === value) {
                        return Promise.resolve()
                      }
                      return Promise.reject(new Error('Passwords do not match'))
                    },
                  }),
                ]}
              >
                <Input.Password
                  autoComplete="new-password"
                  maxLength={72}
                  placeholder="Confirm new password"
                />
              </Form.Item>

              <Divider className="!my-3" />
              <Flex justify="end" gap={8}>
                <Button
                  htmlType="button"
                  disabled={savingPassword}
                  onClick={() => passwordForm.resetFields()}
                >
                  Cancel
                </Button>
                <Button
                  type="primary"
                  htmlType="submit"
                  loading={savingPassword}
                >
                  Change password
                </Button>
              </Flex>
            </Form>
          ) : (
            <Text type="secondary">
              You sign in through an external provider, so there is no password
              to change here.
            </Text>
          )}
        </Card>
      )}
    </SettingsPageContainer>
  )
}
