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
import { FieldError } from '@/components/ui/shadcn/field'
import type { GalleryStory } from '../story'

const noop = () => undefined

/**
 * Trigger-based OPEN + loading Sheet — renders a CLOSED trigger by default (per
 * this file's contract), so `overlays.spec.ts` opens it and asserts the loading
 * arm (`loading ? <Spinner> : children`), an arm no closed-trigger render reaches.
 *
 * It must NOT open on mount: the browse-all canvas mounts every story at once, so
 * an open Sheet portals a `data-slot="sheet-overlay"` backdrop (`fixed inset-0`)
 * over the whole page — it intercepts pointer events (blocking hover/click specs)
 * and its scroll-lock trips the document horizontal-scroll invariant. Uncontrolled
 * (base-ui opens on the trigger, Escape/backdrop dismiss).
 */
function SheetOpenLoadingCase() {
  return (
    <Sheet
      data-testid="g-sheet-loading"
      loading
      loadingLabel="Loading sheet content"
      title="Loading sheet"
      trigger={
        <Button data-testid="g-sheet-loading-open" variant="outline">
          Open loading sheet
        </Button>
      }
    >
      <p className="text-sm text-muted-foreground">Hidden while loading.</p>
    </Sheet>
  )
}

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
    {
      // Loading arm: the body is replaced by a centered spinner
      // (`loading ? <Spinner> : children`) — an arm no closed-trigger render
      // reaches. Renders a CLOSED trigger; overlays.spec.ts opens it to assert
      // the arm, so it never pins a modal backdrop over the browse canvas
      // (see SheetOpenLoadingCase).
      key: 'open-loading',
      label: 'Open · loading',
      render: () => <SheetOpenLoadingCase />,
    },
  ],
}

const fieldErrorStory: GalleryStory = {
  id: 'field-error',
  title: 'FieldError',
  note: 'multi-error list arm (>1 unique messages → the <ul><li> map)',
  cases: [
    {
      key: 'single',
      label: 'Single error',
      render: () => (
        <FieldError
          data-testid="g-field-error-single"
          errors={[{ message: 'This field is required.' }]}
        />
      ),
    },
    {
      // ≥2 unique messages → FieldError renders the bulleted <ul> and maps each
      // `error.message` to an <li> (field.tsx:203) — the single-message arm above
      // returns a bare string and never hits that map.
      key: 'multi',
      label: 'Multiple errors',
      render: () => (
        <FieldError
          data-testid="g-field-error-multi"
          errors={[
            { message: 'Must be at least 8 characters.' },
            { message: 'Must contain a number.' },
          ]}
        />
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
  fieldErrorStory,
  popoverStory,
  confirmStory,
  dropdownStory,
]
