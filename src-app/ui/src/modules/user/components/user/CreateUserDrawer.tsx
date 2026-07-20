import { z } from 'zod'
import { Button, Form, FormField, Input, PasswordInput, message, useForm, zodResolver } from '@ziee/kit'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { Stores } from '@ziee/framework/stores'
import { Users } from '@/modules/user/stores/users'
import { usePermission } from '@/core/permissions'
import type { CreateUserRequest } from '@/api-client/types'
import { Permissions } from '@/api-client/permissions'
import { PermissionsField } from '@/modules/user/components/PermissionsField.tsx'
import { EMAIL_RE } from '@/lib/validation'

const createUserSchema = z.object({
  username: z.string().min(1, 'Please enter username'),
  email: z.string().min(1, 'Please enter valid email').regex(EMAIL_RE, 'Please enter valid email'),
  password: z.string().min(1, 'Please enter password').min(6, 'Password must be at least 6 characters'),
  display_name: z.string().optional(),
  permissions: z.array(z.string()).optional(),
})

type CreateUserValues = z.infer<typeof createUserSchema>

export function CreateUserDrawer() {
  const { isOpen } = Stores.CreateUserDrawer
  const { creating: creatingUser } = Users
  const createForm = useForm<CreateUserValues>({
    resolver: zodResolver(createUserSchema),
    defaultValues: {
      username: '',
      email: '',
      password: '',
      display_name: '',
      permissions: [],
    },
  })
  const canCreate = usePermission(Permissions.UsersCreate)

  const handleCreateUser = async (values: CreateUserValues) => {
    try {
      const userData: CreateUserRequest = {
        username: values.username,
        email: values.email,
        password: values.password,
        display_name: values.display_name,
        permissions: values.permissions?.length
          ? values.permissions
          : undefined,
      }

      await Users.createUser(userData)

      message.success('User created successfully')
      Stores.CreateUserDrawer.closeCreateUserDrawer()
      createForm.reset()
    } catch (error) {
      console.error('Failed to create user:', error)
      // Error is handled by the store
    }
  }

  return (
    <Drawer
      title="Create User"
      open={isOpen}
      onClose={() => {
        Stores.CreateUserDrawer.closeCreateUserDrawer()
        createForm.reset()
      }}
      footer={
        <div className="flex justify-end gap-2">
          <Button
            variant="outline"
            onClick={() => {
              Stores.CreateUserDrawer.closeCreateUserDrawer()
              createForm.reset()
            }}
            disabled={creatingUser}
            data-testid="user-create-cancel-button"
          >
            {canCreate ? 'Cancel' : 'Close'}
          </Button>
          {canCreate && (
            <Button type="submit" form="create-user" loading={creatingUser} data-testid="user-create-submit-button">
              Create
            </Button>
          )}
        </div>
      }
      size={600}
      mask={{ closable: false }}
    >
      <Form
        name="create-user"
        form={createForm}
        layout="vertical"
        onSubmit={handleCreateUser}
        disabled={!canCreate}
        data-testid="user-create-form"
      >
        <FormField
          name="username"
          label="Username"
          required
        >
          <Input placeholder="Enter username" data-testid="user-create-username-input" />
        </FormField>
        <FormField
          name="email"
          label="Email"
          required
        >
          <Input placeholder="Enter email" data-testid="user-create-email-input" />
        </FormField>
        <FormField
          name="password"
          label="Password"
          required
        >
          <PasswordInput placeholder="Enter password" showLabel="Show" hideLabel="Hide" data-testid="user-create-password-input" />
        </FormField>
        <FormField name="display_name" label="Display Name">
          <Input placeholder="Enter display name (optional)" data-testid="user-create-display-name-input" />
        </FormField>
        <FormField name="permissions" label="Permissions">
          <PermissionsField disabled={!canCreate} />
        </FormField>
      </Form>
    </Drawer>
  )
}
