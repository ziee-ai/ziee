import { useEffect } from 'react'
import {
  Avatar,
  Button,
  Card,
  Descriptions,
  Separator,
  Flex,
  Form,
  FormField,
  useForm,
  zodResolver,
  Input,
  PasswordInput,
  Tag,
  Text,
  message,
} from '@/components/ui'
import { z } from 'zod'
import { User } from 'lucide-react'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'

interface ProfileFormValues {
  display_name: string
  username: string
}

interface PasswordFormValues {
  current_password: string
  new_password: string
  confirm_password: string
}

const profileSchema = z.object({
  display_name: z.string(),
  username: z
    .string()
    .min(1, 'Username is required')
    .refine((v) => v.trim().length > 0, 'Username cannot be blank'),
})

const passwordSchema = z
  .object({
    current_password: z.string().min(1, 'Enter your current password'),
    new_password: z
      .string()
      .min(1, 'Enter a new password')
      .min(8, 'Password must be at least 8 characters')
      .max(72, 'Password must be at most 72 characters'),
    confirm_password: z.string().min(1, 'Re-enter the new password'),
  })
  .refine((d) => d.new_password === d.confirm_password, {
    message: 'Passwords do not match',
    path: ['confirm_password'],
  })

export function ProfileSettingsPage() {
  // Read ALL store fields at the top, before any early return (hooks rule).
  const { user, hasPassword } = Stores.Auth
  const { savingProfile, savingPassword } = Stores.Profile
  const canEdit = usePermission(Permissions.ProfileEdit)
  const profileForm = useForm<ProfileFormValues>({
    resolver: zodResolver(profileSchema),
    defaultValues: { display_name: '', username: '' },
  })
  const passwordForm = useForm<PasswordFormValues>({
    resolver: zodResolver(passwordSchema),
    defaultValues: {
      current_password: '',
      new_password: '',
      confirm_password: '',
    },
  })

  // Refresh /me on mount so `hasPassword` + profile fields are accurate
  // even when the user arrived via an in-session login (authenticateUser
  // sets `user` from the login response, which carries no `has_password`).
  useEffect(() => {
    void Stores.Auth.refreshCurrentUser()
  }, [])

  useEffect(() => {
    if (user) {
      profileForm.reset({
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
      passwordForm.reset()
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
        <Flex vertical gap="lg">
        <Flex align="center" gap="md">
          {user.avatar_url ? (
            <Avatar
              className="size-16"
              src={user.avatar_url}
              alt={user.username}
              fallback={<User />}
            />
          ) : (
            <Avatar className="size-16" fallback={<User />} />
          )}
          <Flex gap="sm" wrap>
            <Tag tone={user.is_admin ? 'warning' : undefined}>
              {user.is_admin ? 'Administrator' : 'User'}
            </Tag>
            <Tag tone={user.email_verified ? 'success' : 'warning'}>
              {user.email_verified ? 'Email verified' : 'Email unverified'}
            </Tag>
          </Flex>
        </Flex>

        <Descriptions
          size="sm"
          column={2}
          items={[
            { key: 'email', label: 'Email', children: user.email },
            {
              key: 'member-since',
              label: 'Member since',
              children: new Date(user.created_at).toLocaleDateString(),
            },
            {
              key: 'last-login',
              label: 'Last login',
              children: user.last_login_at
                ? new Date(user.last_login_at).toLocaleDateString()
                : 'Never',
            },
          ]}
        />

        <Form
          name="profile-form"
          form={profileForm}
          layout="horizontal"
          labelWidth={160}
          onSubmit={handleProfileSubmit}
          disabled={!canEdit}
        >
          <FormField
            name="display_name"
            label="Display name"
            description="The name shown to others. Optional."
          >
            <Input maxLength={255} placeholder="Your display name" />
          </FormField>
          <FormField name="username" label="Username" required>
            <Input maxLength={255} placeholder="Your username" />
          </FormField>

          {canEdit && (
            <>
              <Separator className="!my-3" />
              <Flex justify="end">
                <Button type="submit" loading={savingProfile}>
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
              labelWidth={160}
              onSubmit={handlePasswordSubmit}
            >
              <FormField
                name="current_password"
                label="Current password"
                required
              >
                <PasswordInput
                  showLabel="Show password"
                  hideLabel="Hide password"
                  autoComplete="current-password"
                  placeholder="Current password"
                />
              </FormField>
              <FormField name="new_password" label="New password" required>
                <PasswordInput
                  showLabel="Show password"
                  hideLabel="Hide password"
                  autoComplete="new-password"
                  placeholder="New password"
                  maxLength={72}
                />
              </FormField>
              <FormField
                name="confirm_password"
                label="Confirm new password"
                required
              >
                <PasswordInput
                  showLabel="Show password"
                  hideLabel="Hide password"
                  autoComplete="new-password"
                  placeholder="Confirm new password"
                />
              </FormField>

              <Separator className="!my-3" />
              <Flex justify="end">
                <Button type="submit" loading={savingPassword}>
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
