/**
 * Dev-gallery seed for the `skill` module — the Skills-in-conversation dialog,
 * skill detail drawer, import dialog, group→skills assignment, and the
 * ConversationSkillsPanel loading/error/empty seeded surfaces.
 * Auto-discovered by the gallery's runtime registry (`@/dev/gallery/support`);
 * never imported by `module.tsx`, so it is dev-only and tree-shaken from prod.
 */
import type { ModuleGallery } from '@/dev/gallery/support'
import { holdPatch, lazyBound, lazyNamed, lazyProps } from '@/dev/gallery/support'
import { Stores } from '@ziee/framework/stores'
import { llmGroupsList } from '@/dev/gallery/fixtures/llm-providers'
import {
  SKILLS_CONVERSATION_ID,
  skillsAvailable,
  skillsList,
} from '@/dev/gallery/fixtures/skills'
import { SkillDrawer } from '@/modules/skill/stores/skillDrawer'
import { GroupSystemSkillsAssignment } from '@/modules/skill/widgets/groupSystemSkillsAssignmentDrawer'

const noop = () => {}

const group = llmGroupsList.groups[0]

/** Seed the install list + per-conversation available set through the REAL
 *  stores (holdPatch re-asserts over any late mock-API load) so the Skills
 *  dialog renders populated / empty. */
async function seedSkills(
  skills: typeof skillsList,
  available: typeof skillsAvailable,
): Promise<void> {
  const { useSkillStore } = await import('@/modules/skill/stores/skill')
  const { ConversationSkills } = await import(
    '@/modules/skill/stores/conversationSkills'
  )
  await holdPatch(() => {
    useSkillStore.setState({ skills, loading: false } as any)
    ConversationSkills.store.setState({
      available: { [SKILLS_CONVERSATION_ID]: available },
      loading: { [SKILLS_CONVERSATION_ID]: false },
      error: null,
    } as any)
  })
}

/** Seed the loading arm (available undefined + loading true). */
async function seedSkillsLoading(): Promise<void> {
  const { ConversationSkills } = await import(
    '@/modules/skill/stores/conversationSkills'
  )
  await holdPatch(() =>
    ConversationSkills.store.setState({
      available: {},
      loading: { [SKILLS_CONVERSATION_ID]: true },
      error: null,
    } as any),
  )
}

export const gallery: ModuleGallery = {
  overlays: [
    {
      slug: 'overlay-group-skills-assignment',
      surface: 'modules/skill/widgets/GroupSystemSkillsAssignmentDrawer',
      title: 'Group → Skills (drawer)',
      component: lazyNamed(
        () => import('@/modules/skill/widgets/GroupSystemSkillsAssignmentDrawer'),
        'GroupSystemSkillsAssignmentDrawer',
      ),
      open: () => GroupSystemSkillsAssignment.openDrawer(group),
    },
    {
      slug: 'overlay-skills-conversation-loaded',
      surface: 'modules/skill/components/SkillConversationDrawer',
      title: 'Skills in this conversation — populated',
      component: lazyBound(
        () => import('@/modules/skill/components/SkillConversationDrawer'),
        'SkillConversationDrawer',
        { conversationId: SKILLS_CONVERSATION_ID },
      ),
      open: () => {
        Stores.SkillConversationDrawer.openDrawer(SKILLS_CONVERSATION_ID)
        void seedSkills(skillsList, skillsAvailable)
      },
      interactions: [
        {
          name: 'open-detail',
          note: 'click a skill row → the nested SkillDetailDrawer opens WITH conversationId (the "Hide in this conversation" checkbox path)',
          steps: async d => {
            await d.click(`skill-conversation-open-${skillsList[0].id}`)
            await d.wait(400)
          },
        },
      ],
    },
    {
      slug: 'overlay-skills-conversation-empty',
      surface: 'modules/skill/components/SkillConversationDrawer',
      title: 'Skills in this conversation — empty',
      component: lazyBound(
        () => import('@/modules/skill/components/SkillConversationDrawer'),
        'SkillConversationDrawer',
        { conversationId: SKILLS_CONVERSATION_ID },
      ),
      open: () => {
        Stores.SkillConversationDrawer.openDrawer(SKILLS_CONVERSATION_ID)
        void seedSkills([], [])
      },
    },
    {
      slug: 'overlay-skills-conversation-loading',
      surface: 'modules/skill/components/SkillConversationDrawer',
      title: 'Skills in this conversation — loading',
      component: lazyBound(
        () => import('@/modules/skill/components/SkillConversationDrawer'),
        'SkillConversationDrawer',
        { conversationId: SKILLS_CONVERSATION_ID },
      ),
      open: () => {
        Stores.SkillConversationDrawer.openDrawer(SKILLS_CONVERSATION_ID)
        void seedSkillsLoading()
      },
    },
    {
      slug: 'overlay-skill-detail-drawer',
      surface: 'modules/skill/components/SkillDetailDrawer',
      title: 'Skill detail (drawer) — with conversation hide-toggle',
      component: lazyNamed(
        () => import('@/modules/skill/components/SkillDetailDrawer'),
        'SkillDetailDrawer',
      ),
      open: () => {
        void seedSkills(skillsList, skillsAvailable)
        SkillDrawer.open(skillsList[0] as any, SKILLS_CONVERSATION_ID)
      },
    },
    {
      slug: 'overlay-import-skill-dialog',
      surface: 'modules/skill/components/ImportSkillDialog',
      title: 'Import skill (dialog)',
      component: lazyBound(
        () => import('@/modules/skill/components/ImportSkillDialog'),
        'ImportSkillDialog',
        { open: true, onClose: noop },
      ),
    },
  ],
  seeded: [
    {
      slug: 'seeded-conversation-skills-loading',
      title: 'Conversation skills — loading',
      note: 'loading && !available → the load spinner',
      path: '/',
      initialPath: '/',
      component: lazyProps(
        () => import('@/modules/skill/components/ConversationSkillsPanel'),
        'ConversationSkillsPanel',
        { conversationId: 'conv-1' },
      ),
      setup: async () => {
        const { ConversationSkills } = await import(
          '@/modules/skill/stores/conversationSkills'
        )
        await holdPatch(() =>
          ConversationSkills.store.setState({
            available: {},
            loading: { 'conv-1': true },
            error: null,
          } as any),
        )
      },
    },
    {
      slug: 'seeded-conversation-skills-error',
      title: 'Conversation skills — error',
      note: 'error && !available → the error state',
      path: '/',
      initialPath: '/',
      component: lazyProps(
        () => import('@/modules/skill/components/ConversationSkillsPanel'),
        'ConversationSkillsPanel',
        { conversationId: 'conv-1' },
      ),
      setup: async () => {
        const { ConversationSkills } = await import(
          '@/modules/skill/stores/conversationSkills'
        )
        await holdPatch(() =>
          ConversationSkills.store.setState({
            available: {},
            loading: { 'conv-1': false },
            error: 'Failed to load skills.',
          } as any),
        )
      },
    },
    {
      slug: 'seeded-conversation-skills-empty',
      title: 'Conversation skills — empty',
      note: 'available loaded but allRows.length===0 → the empty state',
      path: '/',
      initialPath: '/',
      component: lazyProps(
        () => import('@/modules/skill/components/ConversationSkillsPanel'),
        'ConversationSkillsPanel',
        { conversationId: 'conv-1' },
      ),
      setup: async () => {
        const { ConversationSkills } = await import(
          '@/modules/skill/stores/conversationSkills'
        )
        const { useSkillStore } = await import('@/modules/skill/stores/skill')
        await holdPatch(() => {
          useSkillStore.setState({ skills: [] } as any)
          ConversationSkills.store.setState({
            available: { 'conv-1': [] },
            loading: { 'conv-1': false },
            error: null,
          } as any)
        })
      },
    },
  ],
}
