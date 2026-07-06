/**
 * Overlay open-state entries — drawers/dialogs/menus rendered in their OPEN
 * state with seeded data. Each entry seeds + fires the overlay's store open
 * action on mount, then renders the component; the Base-UI Sheet/Dialog portals
 * to the body, so a full-page screenshot captures it.
 *
 * Rendered ONE-PER-PAGE-LOAD via the URL-isolation path (`?surface=<slug>&state=open`)
 * — like chat detail — so the per-overlay singleton store never bleeds across
 * entries and multiple open portals don't stack on one canvas.
 */
import { type ComponentType, type LazyExoticComponent, lazy } from 'react'
import { Stores } from '@/core/stores'
import { dialog } from '@/components/ui'
import { adminUser } from './fixtures/auth'
import { llmProvidersList, llmGroupsList } from './fixtures/llm-providers'
import type { InteractionRecipe } from './interactions'

export interface OverlayEntry {
  /** Gallery slug → `?surface=<slug>&state=open`; also the section testid. */
  slug: string
  /** Coverage surface id (the component file). */
  surface: string
  /** Human title for the frame. */
  title: string
  component: LazyExoticComponent<ComponentType>
  /** Seed + fire the store open action (runs on mount). */
  open: () => void
  /** Interaction recipes driven after the overlay opens (focus an input, submit
   *  invalid, …). Driven via `?surface=<slug>&interact=<name>`. */
  interactions?: InteractionRecipe[]
}

const provider = llmProvidersList.providers[0]
const group = llmGroupsList.groups[0]

const lazyNamed = (loader: () => Promise<any>, name: string) =>
  lazy(() => loader().then(m => ({ default: m[name] })))

export const OVERLAY_ENTRIES: OverlayEntry[] = [
  {
    slug: 'overlay-llm-provider-drawer',
    surface: 'modules/llm-provider/components/LlmProviderDrawer',
    title: 'Edit LLM Provider (drawer)',
    component: lazyNamed(
      () => import('@/modules/llm-provider/components/LlmProviderDrawer'),
      'LlmProviderDrawer',
    ),
    open: () => Stores.LlmProviderDrawer.openLlmProviderDrawer(provider),
  },
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
  {
    slug: 'overlay-llm-repository-drawer',
    surface: 'modules/llm-repository/components/LlmRepositoryDrawer',
    title: 'LLM Repository (drawer)',
    component: lazyNamed(
      () => import('@/modules/llm-repository/components/LlmRepositoryDrawer'),
      'LlmRepositoryDrawer',
    ),
    open: () => Stores.LlmRepositoryDrawer.openDrawer(),
  },
  {
    slug: 'overlay-group-llm-providers-assignment',
    surface: 'modules/llm-provider/components/GroupLlmProvidersAssignmentDrawer',
    title: 'Group → LLM Providers (drawer)',
    component: lazyNamed(
      () => import('@/modules/llm-provider/components/GroupLlmProvidersAssignmentDrawer'),
      'GroupLlmProvidersAssignmentDrawer',
    ),
    open: () => Stores.GroupLlmProvidersAssignment.openDrawer(group),
  },
  {
    slug: 'overlay-group-mcp-servers-assignment',
    surface: 'modules/mcp/components/system/GroupSystemMcpServersAssignmentDrawer',
    title: 'Group → MCP Servers (drawer)',
    component: lazyNamed(
      () => import('@/modules/mcp/components/system/GroupSystemMcpServersAssignmentDrawer'),
      'GroupSystemMcpServersAssignmentDrawer',
    ),
    open: () => Stores.GroupSystemMcpServersAssignment.openDrawer(group),
  },
  {
    slug: 'overlay-group-skills-assignment',
    surface: 'modules/skill/widgets/GroupSystemSkillsAssignmentDrawer',
    title: 'Group → Skills (drawer)',
    component: lazyNamed(
      () => import('@/modules/skill/widgets/GroupSystemSkillsAssignmentDrawer'),
      'GroupSystemSkillsAssignmentDrawer',
    ),
    open: () => Stores.GroupSystemSkillsAssignment.openDrawer(group),
  },
  {
    slug: 'overlay-group-workflows-assignment',
    surface: 'modules/workflow/widgets/GroupSystemWorkflowsAssignmentDrawer',
    title: 'Group → Workflows (drawer)',
    component: lazyNamed(
      () => import('@/modules/workflow/widgets/GroupSystemWorkflowsAssignmentDrawer'),
      'GroupSystemWorkflowsAssignmentDrawer',
    ),
    open: () => Stores.GroupSystemWorkflowsAssignment.openDrawer(group),
  },
  {
    slug: 'overlay-assistant-form-drawer',
    surface: 'modules/assistant/components/AssistantFormDrawer',
    title: 'Create Assistant (drawer)',
    component: lazyNamed(
      () => import('@/modules/assistant/components/AssistantFormDrawer'),
      'AssistantFormDrawer',
    ),
    open: () => Stores.AssistantDrawer.openAssistantDrawer(),
  },
  {
    // <DialogHost/> singleton with a DESCRIBED alert → the open AlertDialog (:94)
    // + the `description != null` arm of the aria-describedby spread (:95).
    slug: 'overlay-dialog-host-described',
    surface: 'components/ui/kit/dialog-host',
    title: 'Imperative dialog — described',
    component: lazyNamed(() => import('@/components/ui'), 'DialogHost'),
    open: () => {
      void dialog.info({
        title: 'Heads up',
        description: 'A described alert dialog.',
        okText: 'OK',
        testid: 'gallery-dialog-with-desc',
      })
    },
  },
  {
    // Bare alert (no description) → the `description == null` arm of :95
    // (aria-describedby explicitly undefined). Separate frame: two simultaneously
    // -open Radix AlertDialogs don't both mount.
    slug: 'overlay-dialog-host-bare',
    surface: 'components/ui/kit/dialog-host',
    title: 'Imperative dialog — bare (no description)',
    component: lazyNamed(() => import('@/components/ui'), 'DialogHost'),
    open: () => {
      void dialog.warning({
        title: 'Bare alert (no description)',
        okText: 'OK',
        testid: 'gallery-dialog-no-desc',
      })
    },
  },
]

/** Surface ids covered with a delivered open-state entry (for the coverage gate). */
export const WIRED_OVERLAY_SURFACES = new Set(OVERLAY_ENTRIES.map(o => o.surface))

export const overlayBySlug = (slug: string) =>
  OVERLAY_ENTRIES.find(o => o.slug === slug)
