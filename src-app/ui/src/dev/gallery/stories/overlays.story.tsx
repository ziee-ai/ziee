/**
 * Stories for overlay / disclosure components. These render a trigger by default
 * (Playwright opens them and snapshots the open state); where an inline open
 * variant is cheap, it's shown too.
 */
import {
  Button,
  Confirm,
  Dialog,
  Dropdown,
  Popover,
  Sheet,
} from '@/components/ui'
import type { GalleryStory } from '../story'

const noop = () => undefined

const dialogStory: GalleryStory = {
  id: 'dialog',
  title: 'Dialog',
  cases: [
    {
      key: 'trigger',
      label: 'Trigger',
      render: () => (
        <Dialog
          data-testid="g-dialog"
          title="Dialog title"
          description="A short description of what this dialog is for."
          trigger={
            <Button data-testid="g-dialog-open" variant="outline">
              Open dialog
            </Button>
          }
          footer={
            <Button data-testid="g-dialog-ok" size="default">
              Confirm
            </Button>
          }
        >
          <p className="text-sm text-muted-foreground">Dialog body content.</p>
        </Dialog>
      ),
    },
  ],
}

const sheetStory: GalleryStory = {
  id: 'sheet',
  title: 'Sheet',
  cases: [
    {
      key: 'trigger',
      label: 'Trigger',
      render: () => (
        <Sheet
          data-testid="g-sheet"
          title="Sheet title"
          description="Slide-over panel."
          trigger={
            <Button data-testid="g-sheet-open" variant="outline">
              Open sheet
            </Button>
          }
        >
          <p className="text-sm text-muted-foreground">Sheet body content.</p>
        </Sheet>
      ),
    },
  ],
}

const popoverStory: GalleryStory = {
  id: 'popover',
  title: 'Popover',
  cases: [
    {
      key: 'trigger',
      label: 'Trigger',
      render: () => (
        <Popover
          title="Popover"
          content={
            <p className="text-sm">Some contextual content in a popover.</p>
          }
        >
          <Button data-testid="g-popover-open" variant="outline">
            Open popover
          </Button>
        </Popover>
      ),
    },
  ],
}

const confirmStory: GalleryStory = {
  id: 'confirm',
  title: 'Confirm',
  cases: [
    {
      key: 'trigger',
      label: 'Trigger',
      render: () => (
        <Confirm
          data-testid="g-confirm"
          title="Delete this item?"
          description="This action cannot be undone."
          okText="Delete"
          cancelText="Cancel"
          danger
          onConfirm={noop}
        >
          <Button data-testid="g-confirm-open" variant="destructive">
            Delete
          </Button>
        </Confirm>
      ),
    },
  ],
}

const dropdownStory: GalleryStory = {
  id: 'dropdown',
  title: 'Dropdown',
  cases: [
    {
      key: 'trigger',
      label: 'Trigger',
      render: () => (
        <Dropdown
          data-testid="g-dropdown"
          items={[
            { key: 'edit', label: 'Edit' },
            { key: 'dup', label: 'Duplicate' },
            { type: 'divider' },
            { key: 'del', label: 'Delete', danger: true },
          ]}
        >
          <Button data-testid="g-dropdown-open" variant="outline">
            Actions
          </Button>
        </Dropdown>
      ),
    },
  ],
}

export const overlayStories: GalleryStory[] = [
  dialogStory,
  sheetStory,
  popoverStory,
  confirmStory,
  dropdownStory,
]
