/**
 * Dev-gallery seed for the `user` module — user + user-group management drawers.
 * Auto-discovered by the gallery's runtime registry (`@/dev/gallery/support`);
 * never imported by `module.tsx`, so it is dev-only and tree-shaken from prod.
 */
import type { ModuleGallery } from '@/dev/gallery/support'
import { lazyNamed } from '@/dev/gallery/support'
import { Stores } from '@ziee/framework/stores'
import { adminUser } from '@/dev/gallery/fixtures/auth'
import { llmGroupsList } from '@/dev/gallery/fixtures/llm-providers'

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
      open: () => Stores.CreateUserDrawer.openCreateUserDrawer(),
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
      open: () => Stores.EditUserDrawer.openEditUserDrawer(adminUser),
    },
    {
      slug: 'overlay-reset-password-drawer',
      surface: 'modules/user/components/user/ResetPasswordDrawer',
      title: 'Reset Password (drawer)',
      component: lazyNamed(
        () => import('@/modules/user/components/user/ResetPasswordDrawer'),
        'ResetPasswordDrawer',
      ),
      open: () => Stores.ResetPasswordDrawer.openResetPasswordDrawer(adminUser),
    },
    {
      slug: 'overlay-edit-user-group-drawer',
      surface: 'modules/user/components/group/EditUserGroupDrawer',
      title: 'Edit User Group (drawer)',
      component: lazyNamed(
        () => import('@/modules/user/components/group/EditUserGroupDrawer'),
        'EditUserGroupDrawer',
      ),
      open: () => Stores.EditUserGroupDrawer.openUserGroupDrawer(group),
    },
    {
      slug: 'overlay-assign-group-drawer',
      surface: 'modules/user/components/user/AssignGroupDrawer',
      title: 'Assign Group (drawer)',
      component: lazyNamed(
        () => import('@/modules/user/components/user/AssignGroupDrawer'),
        'AssignGroupDrawer',
      ),
      open: () => Stores.AssignGroupDrawer.openAssignGroupDrawer(adminUser),
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
      surface: 'modules/user/components/user/UserGroupsDrawer',
      title: 'User Groups (drawer)',
      component: lazyNamed(
        () => import('@/modules/user/components/user/UserGroupsDrawer'),
        'UserGroupsDrawer',
      ),
      open: () => Stores.UserGroupsDrawer.openUserGroupsDrawer(adminUser),
    },
    {
      slug: 'overlay-group-members-drawer',
      surface: 'modules/user/components/group/GroupMembersDrawer',
      title: 'Group Members (drawer)',
      component: lazyNamed(
        () => import('@/modules/user/components/group/GroupMembersDrawer'),
        'GroupMembersDrawer',
      ),
      open: () => Stores.GroupMembersDrawer.openGroupMembersDrawer(group),
    },
  ],
}
