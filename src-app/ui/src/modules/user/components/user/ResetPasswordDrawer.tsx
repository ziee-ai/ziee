import { z } from 'zod'
import { Button, Form, FormField, useForm, zodResolver, PasswordInput, message } from '@ziee/kit'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { Stores } from '@ziee/framework/stores'
import { Users } from '@/modules/user/stores/Users.store'
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
      await Users.resetUserPassword(user.id, values.new_password)

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
      footer={
        <div className="flex justify-end gap-2">
          <Button
            variant="outline"
            onClick={() => {
              Stores.ResetPasswordDrawer.closeResetPasswordDrawer()
              form.reset()
            }}
            data-testid="user-reset-password-cancel-button"
          >
            {canReset ? 'Cancel' : 'Close'}
          </Button>
          {canReset && (
            <Button type="submit" form="reset-password-form" data-testid="user-reset-password-submit-button">
              Reset
            </Button>
          )}
        </div>
      }
      mask={{ closable: false }}
    >
      <Form
        name="reset-password-form"
        form={form}
        layout="vertical"
        onSubmit={handleResetPassword}
        disabled={!canReset}
        data-testid="user-reset-password-form"
      >
        <FormField
          name="new_password"
          label="New Password"
        >
          <PasswordInput placeholder="Enter new password" showLabel="Show password" hideLabel="Hide password" data-testid="user-reset-new-password-input" />
        </FormField>
        <FormField
          name="confirm_password"
          label="Confirm Password"
        >
          <PasswordInput placeholder="Confirm new password" showLabel="Show password" hideLabel="Hide password" data-testid="user-reset-confirm-password-input" />
        </FormField>
      </Form>
    </Drawer>
  )
}
