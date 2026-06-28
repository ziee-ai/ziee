import { z } from 'zod'
import { Button, Flex, Form, FormField, useForm, zodResolver, PasswordInput, message } from '@/components/ui'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'

const resetPasswordSchema = z
  .object({
    new_password: z
      .string()
      .min(1, 'Please enter new password')
      .min(6, 'Password must be at least 6 characters'),
    confirm_password: z.string().min(1, 'Please confirm password'),
  })
  .refine((d) => d.confirm_password === d.new_password, {
    message: 'Passwords do not match',
    path: ['confirm_password'],
  })

type ResetPasswordValues = z.infer<typeof resetPasswordSchema>

export function ResetPasswordDrawer() {
  const { isOpen, user } = Stores.ResetPasswordDrawer
  const canReset = usePermission(Permissions.UsersResetPassword)
  const form = useForm<ResetPasswordValues>({
    resolver: zodResolver(resetPasswordSchema),
    defaultValues: { new_password: '', confirm_password: '' },
  })

  const handleResetPassword = async (values: ResetPasswordValues) => {
    if (!user) return

    try {
      await Stores.Users.resetUserPassword(user.id, values.new_password)

      message.success('Password reset successfully')
      Stores.ResetPasswordDrawer.closeResetPasswordDrawer()
      form.reset()
    } catch (error) {
      console.error('Failed to reset password:', error)
      // Error is handled by the store
    }
  }

  return (
    <Drawer
      title="Reset Password"
      size={600}
      open={isOpen}
      onClose={() => {
        Stores.ResetPasswordDrawer.closeResetPasswordDrawer()
        form.reset()
      }}
      footer={null}
      mask={{ closable: false }}
    >
      <Form
        form={form}
        layout="vertical"
        onSubmit={handleResetPassword}
        disabled={!canReset}
      >
        <FormField
          name="new_password"
          label="New Password"
        >
          <PasswordInput placeholder="Enter new password" showLabel="Show password" hideLabel="Hide password" />
        </FormField>
        <FormField
          name="confirm_password"
          label="Confirm Password"
        >
          <PasswordInput placeholder="Confirm new password" showLabel="Show password" hideLabel="Hide password" />
        </FormField>
        <div className="mb-0">
          <Flex className="justify-end gap-2">
            <Button
              variant="outline"
              onClick={() => {
                Stores.ResetPasswordDrawer.closeResetPasswordDrawer()
                form.reset()
              }}
            >
              {canReset ? 'Cancel' : 'Close'}
            </Button>
            {canReset && (
              <Button type="submit">
                Reset
              </Button>
            )}
          </Flex>
        </div>
      </Form>
    </Drawer>
  )
}
