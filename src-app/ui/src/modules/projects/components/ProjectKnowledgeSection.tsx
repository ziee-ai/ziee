import { useState } from 'react'
import { Button, Spin, Text } from '@/components/ui'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { Stores } from '@/core/stores'
import {
  DrawerOpenerProvider,
  ProjectExtensionSlot,
} from '@/modules/projects/core/extensions'

export function ProjectKnowledgeSection() {
  const [open, setOpen] = useState(false)
  const openDrawer = () => setOpen(true)
  const loading = Stores.ProjectDetail.loading
  const project = Stores.ProjectDetail.project

  // Don't render anything when no project is loaded and nothing is loading
  // (defense-in-depth — the parent page typically gates on this already).
  if (!project && !loading) return null

  return (
    <DrawerOpenerProvider open={openDrawer}>
      {/* Section header — title left, Manage button pushed to the
          right via ml-auto. aria-label preserved exactly so the
          existing E2E spec (attach-file.spec.ts) keeps matching. */}
      <div className="flex items-center mb-2">
        <Text strong>Project knowledge</Text>
        <Button
          data-testid="project-knowledge-manage-button"
          size="sm"
          onClick={openDrawer}
          aria-label="Manage knowledge files"
          className="!ml-auto"
        >
          Manage
        </Button>
      </div>

      {/* Inline preview surface — stacks all kinds' inlinePreview
          components top-to-bottom (file today; URLs/notes/etc. in
          the future). */}
      <div className="flex flex-col gap-4">
        {loading ? (
          <div className="flex justify-center py-6">
            <Spin label="Loading knowledge" />
          </div>
        ) : (
          <ProjectExtensionSlot
            name="knowledge_kinds"
            view="inlinePreview"
          />
        )}
      </div>

      {/* Management drawer — stacks all kinds' managePanel components
          top-to-bottom. When more kinds exist, each gets its own section
          inside the drawer (no tabs). */}
      <Drawer
        title="Project knowledge"
        open={open}
        onClose={() => setOpen(false)}
        size={600}
        destroyOnHidden
        footer={null}
      >
        <div className="flex flex-col gap-6">
          <ProjectExtensionSlot
            name="knowledge_kinds"
            view="managePanel"
          />
        </div>
      </Drawer>
    </DrawerOpenerProvider>
  )
}
