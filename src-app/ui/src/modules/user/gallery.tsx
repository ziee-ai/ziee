/**
 * Dev-gallery seed for the `user` module — user + user-group management drawers.
 * Auto-discovered by the gallery's runtime registry (`@/dev/gallery/support`);
 * never imported by `module.tsx`, so it is dev-only and tree-shaken from prod.
 */
import type { ModuleGallery } from '@/dev/gallery/support'
import { lazyNamed } from '@/dev/gallery/support'
import { adminUser } from '@/dev/gallery/fixtures/auth'
import { llmGroupsList } from '@/dev/gallery/fixtures/llm-providers'
import { EditUserDrawer as EditUserDrawerStore } from '@/modules/user/components/user/editUserDrawer'
import { ResetPasswordDrawer as ResetPasswordDrawerStore } from '@/modules/user/components/user/resetPasswordDrawer'
import { AssignGroupDrawer as AssignGroupDrawerStore } from '@/modules/user/components/user/assignGroupDrawer'
import { UserGroupsDrawer as UserGroupsDrawerStore } from '@/modules/user/components/user/userGroupsDrawer'
import { CreateUserDrawer as CreateUserDrawerStore } from '@/modules/user/components/user/createUserDrawer'
import { GroupMembersDrawer as GroupMembersDrawerStore } from '@/modules/user/components/group/groupMembersDrawer'
import { EditUserGroupDrawer as EditUserGroupDrawerStore } from '@/modules/user/components/group/editUserGroupDrawer'

const group = llmGroupsList.groups[0]

export const gallery: ModuleGallery = {
  overlays: [
    {
      slug: 'overlay-create-user-drawer',
      surface: 'modules/user/components/user/CreateUserDrawer',
      title: 'Create User (drawer)',
      component: lazyNamed(
        () => import('@/modules/user/components/user/CreateUserDrawer'),
        'CreateUserDrawer',
      ),
      open: () => CreateUserDrawerStore.openCreateUserDrawer(),
      interactions: [
        {
          name: 'focus-input',
          note: 'focus the username field → the :focus-visible ring (drives G7: ring clipping / offset in a dense drawer form)',
          steps: async d => {
            await d.focus('user-create-username-input')
          },
        },
        {
          name: 'submit-invalid',
          note: 'submit the empty form → inline required-field validation (G6 error state per input)',
          steps: async d => {
            await d.click('user-create-submit-button')
            await d.wait(300)
          },
        },
      ],
    },
    {
      slug: 'overlay-edit-user-drawer',
      surface: 'modules/user/components/user/EditUserDrawer',
      title: 'Edit User (drawer)',
      component: lazyNamed(
        () => import('@/modules/user/components/user/EditUserDrawer'),
        'EditUserDrawer',
      ),
      open: () => EditUserDrawerStore.openEditUserDrawer(adminUser),
    },
    {
      slug: 'overlay-reset-password-drawer',
      surface: 'modules/user/components/user/ResetPasswordDrawer',
      title: 'Reset Password (drawer)',
      component: lazyNamed(
        () => import('@/modules/user/components/user/ResetPasswordDrawer'),
        'ResetPasswordDrawer',
      ),
      open: () => ResetPasswordDrawerStore.openResetPasswordDrawer(adminUser),
    },
    {
      slug: 'overlay-edit-user-group-drawer',
      surface: 'modules/user/components/group/EditUserGroupDrawer',
      title: 'Edit User Group (drawer)',
      component: lazyNamed(
        () => import('@/modules/user/components/group/EditUserGroupDrawer'),
        'EditUserGroupDrawer',
      ),
      open: () => EditUserGroupDrawerStore.openUserGroupDrawer(group),
    },
    {
      slug: 'overlay-assign-group-drawer',
      surface: 'modules/user/components/user/AssignGroupDrawer',
      title: 'Assign Group (drawer)',
      component: lazyNamed(
        () => import('@/modules/user/components/user/AssignGroupDrawer'),
        'AssignGroupDrawer',
      ),
      open: () => AssignGroupDrawerStore.openAssignGroupDrawer(adminUser),
      interactions: [
        {
          name: 'submit-empty',
          note: 'submit with no group selected → the handleAssignGroup empty-selection guard (AssignGroupDrawer.tsx:63)',
          steps: async d => {
            await d.click('user-assign-group-submit-button')
            await d.wait(200)
          },
        },
      ],
    },
    {
      slug: 'overlay-user-groups-drawer',
      surface: 'modules/user/components/user/userGroupsDrawer',
      title: 'User Groups (drawer)',
      component: lazyNamed(
        () => import('@/modules/user/components/user/UserGroupsDrawer'),
        'UserGroupsDrawer',
      ),
      open: () => UserGroupsDrawerStore.openUserGroupsDrawer(adminUser),
    },
    {
      slug: 'overlay-group-members-drawer',
      surface: 'modules/user/components/group/GroupMembersDrawer',
      title: 'Group Members (drawer)',
      component: lazyNamed(
        () => import('@/modules/user/components/group/GroupMembersDrawer'),
        'GroupMembersDrawer',
      ),
      open: () => GroupMembersDrawerStore.openGroupMembersDrawer(group),
    },
  ],
}
