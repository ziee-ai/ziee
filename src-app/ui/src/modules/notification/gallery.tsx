/**
 * Dev-gallery seed for the `notification` module — the `/notifications` inbox
 * page + the sidebar bell unread badge. Auto-discovered by the gallery's runtime
 * registry (`@/dev/gallery/support`); never imported by `module.tsx`, so it is
 * dev-only and tree-shaken from prod.
 */
import type { ModuleGallery } from '@/dev/gallery/support'

const USER = 'aaaa0000-0000-0000-0000-000000000001'

export const gallery: ModuleGallery = {
  cassette: {
    'Notification.list': {
      items: [
        {
          id: 'n0000000-0000-0000-0000-000000000001',
          title: 'Scheduled task completed',
          body: 'Your "Weekly literature scan" run finished — 3 new papers found.',
          kind: 'scheduled_task',
          interrupt: true,
          created_at: '2026-01-03T09:15:00.000Z',
          scheduled_task_id: 's0000000-0000-0000-0000-000000000001',
          user_id: USER,
        },
        {
          id: 'n0000000-0000-0000-0000-000000000002',
          title: 'Workflow run needs your attention',
          body: 'The "Grant summary" workflow paused waiting for approval.',
          kind: 'workflow',
          interrupt: false,
          created_at: '2026-01-02T14:02:00.000Z',
          read_at: '2026-01-02T14:30:00.000Z',
          workflow_run_id: 'w0000000-0000-0000-0000-000000000001',
          user_id: USER,
        },
        {
          id: 'n0000000-0000-0000-0000-000000000003',
          title: 'Model download finished',
          body: 'whisper base.en is now installed and ready for dictation.',
          kind: 'system',
          interrupt: false,
          created_at: '2026-01-01T18:45:00.000Z',
          user_id: USER,
        },
      ],
      page: 1,
      per_page: 20,
      total: 3,
      unread: 1,
    },
    'Notification.unreadCount': { unread: 1 },
  },
}
