/**
 * Stories for display / status / feedback components.
 */
import { CheckCircle2, Inbox } from 'lucide-react'
import {
  Accordion,
  Alert,
  Avatar,
  Badge,
  Button,
  Card,
  Empty,
  ErrorState,
  Progress,
  Result,
  SectionHeader,
  Separator,
  Skeleton,
  Spin,
  Spinner,
  Statistic,
  Tabs,
  Tag,
  Tooltip,
} from '@/components/ui'
import type { TagTone } from '@/components/ui'
import type { GalleryStory } from '../story'

const noop = () => undefined

const tagTones: TagTone[] = [
  'default',
  'primary',
  'success',
  'warning',
  'error',
  'info',
]

const tagStory: GalleryStory = {
  id: 'tag',
  title: 'Tag',
  note: 'tone × variant; closable',
  cases: [
    {
      key: 'soft',
      label: 'Soft (tones)',
      render: () => (
        <>
          {tagTones.map(t => (
            <Tag key={t} data-testid={`g-tag-soft-${t}`} tone={t}>
              {t}
            </Tag>
          ))}
        </>
      ),
    },
    {
      key: 'solid',
      label: 'Solid',
      render: () => (
        <>
          {tagTones.map(t => (
            <Tag
              key={t}
              data-testid={`g-tag-solid-${t}`}
              tone={t}
              variant="solid"
            >
              {t}
            </Tag>
          ))}
        </>
      ),
    },
    {
      key: 'outline',
      label: 'Outline',
      render: () => (
        <>
          {tagTones.map(t => (
            <Tag
              key={t}
              data-testid={`g-tag-outline-${t}`}
              tone={t}
              variant="outline"
            >
              {t}
            </Tag>
          ))}
        </>
      ),
    },
    {
      key: 'closable',
      label: 'Closable',
      render: () => (
        <Tag
          data-testid="g-tag-closable"
          tone="info"
          closeLabel="Remove tag"
          onClose={noop}
        >
          Closable
        </Tag>
      ),
    },
  ],
}

const badgeStory: GalleryStory = {
  id: 'badge',
  title: 'Badge',
  cases: [
    {
      key: 'count',
      label: 'Count / dot / overflow',
      render: () => (
        <div className="flex gap-6 items-center">
          <Badge data-testid="g-badge-count" count={5} aria-label="5 items">
            <Avatar fallback="A" />
          </Badge>
          <Badge data-testid="g-badge-dot" dot aria-label="New">
            <Avatar fallback="B" />
          </Badge>
          <Badge
            data-testid="g-badge-overflow"
            count={120}
            overflowCount={99}
            aria-label="120 items"
          >
            <Avatar fallback="C" />
          </Badge>
        </div>
      ),
    },
  ],
}

const alertStory: GalleryStory = {
  id: 'alert',
  title: 'Alert',
  cases: [
    {
      key: 'tones',
      label: 'Tones',
      render: () => (
        <div className="flex flex-col gap-2 w-80">
          {(['info', 'success', 'warning', 'error'] as const).map(t => (
            <Alert
              key={t}
              data-testid={`g-alert-${t}`}
              tone={t}
              title={`${t} title`}
              description={`A ${t} alert with a short description.`}
            />
          ))}
          <Alert
            data-testid="g-alert-closable"
            tone="info"
            title="Closable"
            closeLabel="Dismiss"
            onClose={noop}
          />
        </div>
      ),
    },
  ],
}

const errorStateStory: GalleryStory = {
  id: 'error-state',
  title: 'Error state',
  cases: [
    {
      key: 'inline',
      label: 'Inline (with retry + details)',
      render: () => (
        <div className="w-96">
          <ErrorState
            data-testid="g-error-state-inline"
            resource="skills"
            description="Something went wrong while loading your skills."
            details="500 Internal Server Error"
            onRetry={noop}
          />
        </div>
      ),
    },
    {
      key: 'no-retry',
      label: 'No retry',
      render: () => (
        <div className="w-96">
          <ErrorState
            data-testid="g-error-state-no-retry"
            resource="the audit log"
            description="This resource can't be re-fetched right now."
          />
        </div>
      ),
    },
    {
      key: 'page',
      label: 'Page variant',
      render: () => (
        <div className="h-72 w-full border border-border rounded-md">
          <ErrorState
            data-testid="g-error-state-page"
            variant="page"
            resource="the hub catalog"
            description="We couldn't reach the hub. Check your connection and try again."
            onRetry={noop}
          />
        </div>
      ),
    },
  ],
}

const avatarStory: GalleryStory = {
  id: 'avatar',
  title: 'Avatar',
  cases: [
    {
      key: 'sizes',
      label: 'Sizes / image / fallback',
      render: () => (
        <div className="flex gap-3 items-center">
          <Avatar size="sm" fallback="S" />
          <Avatar fallback="M" />
          <Avatar size="lg" fallback="L" />
          <Avatar fallback={<CheckCircle2 />} />
          <Avatar
            src={
              'data:image/svg+xml,' +
              encodeURIComponent(
                '<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><rect width="40" height="40" fill="%2330a46c"/></svg>',
              )
            }
            alt="Avatar image"
          />
          {/* broken src → fallback letter */}
          <Avatar src="data:image/png;base64,broken" alt="Broken" fallback="B" />
        </div>
      ),
    },
  ],
}

const progressStory: GalleryStory = {
  id: 'progress',
  title: 'Progress',
  cases: [
    {
      key: 'line',
      label: 'Line (tones)',
      render: () => (
        <div className="flex flex-col gap-3 w-72">
          {(['primary', 'success', 'warning', 'error'] as const).map(t => (
            <Progress
              key={t}
              data-testid={`g-prog-${t}`}
              aria-label={`${t} progress`}
              tone={t}
              value={t === 'error' ? 30 : 65}
              showInfo
            />
          ))}
        </div>
      ),
    },
    {
      key: 'circle',
      label: 'Circle',
      render: () => (
        <Progress
          data-testid="g-prog-circle"
          aria-label="Circular progress"
          shape="circle"
          value={75}
          circleSize={80}
        />
      ),
    },
  ],
}

const spinnerStory: GalleryStory = {
  id: 'spinner',
  title: 'Spinner / Spin',
  cases: [
    {
      key: 'sizes',
      label: 'Spinner sizes',
      render: () => (
        <div className="flex gap-4 items-center">
          <Spinner size="sm" label="Loading small" />
          <Spinner label="Loading" />
          <Spinner size="lg" label="Loading large" />
        </div>
      ),
    },
    {
      key: 'spin',
      label: 'Spin (wrapper)',
      render: () => (
        <Spin label="Loading content" spinning description="Fetching…">
          <div className="h-16 w-40 rounded-md border border-border bg-muted" />
        </Spin>
      ),
    },
  ],
}

const statisticStory: GalleryStory = {
  id: 'statistic',
  title: 'Statistic',
  cases: [
    {
      key: 'basic',
      label: 'Basic',
      render: () => (
        <div className="flex gap-8">
          <Statistic
            data-testid="g-stat-users"
            title="Active users"
            value={1128}
            groupSeparator
          />
          <Statistic
            data-testid="g-stat-rate"
            title="Success rate"
            value={93.2}
            precision={1}
            suffix="%"
          />
        </div>
      ),
    },
  ],
}

const emptyStory: GalleryStory = {
  id: 'empty',
  title: 'Empty',
  cases: [
    {
      key: 'default',
      label: 'With action',
      render: () => (
        <Empty
          data-testid="g-empty"
          icon={<Inbox />}
          title="No results"
          description="Try adjusting your filters."
        >
          <Button data-testid="g-empty-action" size="default">
            Create
          </Button>
        </Empty>
      ),
    },
  ],
}


const resultStory: GalleryStory = {
  id: 'result',
  title: 'Result',
  cases: [
    {
      key: 'success',
      label: 'Success',
      render: () => (
        <Result
          data-testid="g-result-success"
          status="success"
          title="Submitted successfully"
          subtitle="Your changes are saved."
          extra={
            <Button data-testid="g-result-action" size="default">
              Continue
            </Button>
          }
        />
      ),
    },
  ],
}

const separatorStory: GalleryStory = {
  id: 'separator',
  title: 'Separator',
  cases: [
    {
      key: 'variants',
      label: 'Plain / labeled / vertical',
      render: () => (
        <div className="flex flex-col gap-3 w-72">
          <Separator />
          <Separator>OR</Separator>
          <div className="flex h-8 items-center gap-3">
            <span className="text-sm">Left</span>
            <Separator orientation="vertical" />
            <span className="text-sm">Right</span>
          </div>
        </div>
      ),
    },
  ],
}

const skeletonStory: GalleryStory = {
  id: 'skeleton',
  title: 'Skeleton',
  cases: [
    {
      key: 'lines',
      label: 'Lines',
      render: () => (
        <div className="flex flex-col gap-2 w-64">
          <Skeleton className="h-4 w-full" />
          <Skeleton className="h-4 w-5/6" />
          <Skeleton className="h-4 w-2/3" />
        </div>
      ),
    },
  ],
}

const cardStory: GalleryStory = {
  id: 'card',
  title: 'Card',
  cases: [
    {
      key: 'variants',
      label: 'Title / extra / footer / sizes',
      render: () => (
        <div className="flex flex-wrap gap-4">
          <Card
            data-testid="g-card-basic"
            title="Card title"
            extra={
              <Button data-testid="g-card-extra" size="default" variant="ghost">
                More
              </Button>
            }
            className="w-64"
          >
            <p className="text-sm text-muted-foreground">
              Body content goes here with a sentence or two.
            </p>
          </Card>
          <Card
            data-testid="g-card-footer"
            title="With footer"
            size="sm"
            footer={
              <Button data-testid="g-card-ok" size="default">
                Action
              </Button>
            }
            className="w-64"
          >
            <p className="text-sm text-muted-foreground">Compact card.</p>
          </Card>
        </div>
      ),
    },
  ],
}

const tabsStory: GalleryStory = {
  id: 'tabs',
  title: 'Tabs',
  cases: [
    {
      key: 'default',
      label: 'Default',
      render: () => (
        <div className="w-96">
          <Tabs
            data-testid="g-tabs"
            defaultValue="one"
            items={[
              { key: 'one', label: 'Overview', children: <p>Overview panel</p> },
              { key: 'two', label: 'Details', children: <p>Details panel</p> },
              {
                key: 'three',
                label: 'Disabled',
                disabled: true,
                children: <p>—</p>,
              },
            ]}
          />
        </div>
      ),
    },
    {
      key: 'editable',
      label: 'Editable (add/close cards)',
      render: () => (
        <div className="w-96">
          <Tabs
            data-testid="g-tabs-editable"
            defaultValue="a"
            editable
            addLabel="Add tab"
            onEdit={() => undefined}
            items={[
              { key: 'a', label: 'Tab A', children: <p>A</p> },
              { key: 'b', label: 'Tab B', children: <p>B</p> },
            ]}
          />
        </div>
      ),
    },
    {
      key: 'sizes',
      label: 'Small',
      render: () => (
        <div className="w-96">
          <Tabs
            data-testid="g-tabs-sm"
            size="sm"
            defaultValue="x"
            items={[
              { key: 'x', label: 'One', children: <p>One</p> },
              { key: 'y', label: 'Two', children: <p>Two</p> },
            ]}
          />
        </div>
      ),
    },
  ],
}

const accordionStory: GalleryStory = {
  id: 'accordion',
  title: 'Accordion',
  cases: [
    {
      key: 'single',
      label: 'Single',
      render: () => (
        <div className="w-96">
          <Accordion
            data-testid="g-accordion"
            type="single"
            defaultValue="a"
            items={[
              { key: 'a', label: 'Section A', children: <p>Content A</p> },
              { key: 'b', label: 'Section B', children: <p>Content B</p> },
            ]}
          />
        </div>
      ),
    },
  ],
}

const tooltipStory: GalleryStory = {
  id: 'tooltip',
  title: 'Tooltip',
  cases: [
    {
      key: 'default',
      label: 'On a button',
      render: () => (
        <Tooltip content="Tooltip content">
          <Button data-testid="g-tooltip-trigger" variant="outline">
            Hover me
          </Button>
        </Tooltip>
      ),
    },
  ],
}

const sectionHeaderStory: GalleryStory = {
  id: 'section-header',
  title: 'Section header',
  note: 'title + actions, never-wrap-with-room (title truncates; actions never drop to a new line)',
  cases: [
    {
      key: 'short',
      label: 'Short title + icon action',
      render: () => (
        <div className="w-80 rounded-lg border border-border p-3">
          <SectionHeader
            data-testid="g-section-header-short"
            title="Template Assistants"
            actions={
              <Button size="icon" variant="default" data-testid="g-section-header-short-add" aria-label="Add">
                +
              </Button>
            }
          />
        </div>
      ),
    },
    {
      key: 'long',
      label: 'Long title truncates (stays one row)',
      render: () => (
        <div className="w-64 rounded-lg border border-border p-3">
          <SectionHeader
            data-testid="g-section-header-long"
            title="An extremely long section header title that must truncate rather than wrap or push the action button onto a second row"
            actions={
              <Button size="icon" variant="default" data-testid="g-section-header-long-add" aria-label="Add">
                +
              </Button>
            }
          />
        </div>
      ),
    },
  ],
}

export const displayStories: GalleryStory[] = [
  tagStory,
  sectionHeaderStory,
  badgeStory,
  alertStory,
  avatarStory,
  progressStory,
  spinnerStory,
  statisticStory,
  emptyStory,
  errorStateStory,
  resultStory,
  separatorStory,
  skeletonStory,
  cardStory,
  tabsStory,
  accordionStory,
  tooltipStory,
]
