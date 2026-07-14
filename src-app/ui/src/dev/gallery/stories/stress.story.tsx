/**
 * Content-stress stories — the highest-yield bug class per visual-testing prior
 * art (Chromatic / EightShapes): components break under EXTREME content, not the
 * tidy demo string. Each case forces a torture input inside a deliberately narrow
 * container so overflow / truncation / wrap / clipping failures surface:
 *   - a long UNBROKEN token (URL/hash) — the classic horizontal-overflow trigger
 *   - long wrapped prose — measures leading/wrapping/height growth
 *   - i18n-expanded text (German/Finnish run ~35% longer than English)
 *   - empty + loading states — skeletons, empty illustrations, zero-data tables
 *   - extreme numbers — grouping + width
 *
 * These sections are screenshot (Layer B) + judged (Layer C) targets; Layer A
 * also runs over them, so a component that overflows ugly under long content is
 * flagged deterministically.
 */
import {
  Alert,
  Button,
  Card,
  Descriptions,
  Input,
  List,
  Menu,
  Select,
  Statistic,
  Table,
  Tag,
} from '@ziee/kit'
import type { ReactNode } from 'react'
import type { TableColumn } from '@ziee/kit'
import type { GalleryStory } from '../story'

const LONG_TOKEN =
  'https://example.com/very/long/unbroken/path/segment?token=abcdefghijklmnopqrstuvwxyz0123456789'
const LONG_PROSE =
  'This is a deliberately long run of body copy meant to wrap across several lines so we can check leading, measure, and whether the container grows gracefully or clips its content unexpectedly.'
const I18N_LONG =
  'Benutzerkontoeinstellungen-Überprüfungsbenachrichtigung' // German-style compound
const HUGE_NUMBER = 1234567890

const longContainer = (children: ReactNode, w = 'w-48') => (
  <div className={`${w} rounded-md border border-border p-2`}>{children}</div>
)

const buttonStress: GalleryStory = {
  id: 'stress-button',
  title: 'Stress — Button',
  note: 'long label in a narrow container; i18n-expanded',
  cases: [
    {
      key: 'long',
      label: 'Long label / narrow',
      render: () =>
        longContainer(
          <Button data-testid="g-stress-btn-long" block>
            {I18N_LONG}
          </Button>,
        ),
    },
    {
      key: 'token',
      label: 'Unbroken token',
      render: () =>
        longContainer(
          <Button data-testid="g-stress-btn-token" block>
            {LONG_TOKEN}
          </Button>,
        ),
    },
  ],
}

const tagStress: GalleryStory = {
  id: 'stress-tag',
  title: 'Stress — Tag',
  cases: [
    {
      key: 'long',
      label: 'Long / unbroken',
      render: () => (
        <div className="flex flex-wrap gap-2 w-56">
          <Tag data-testid="g-stress-tag-long" tone="info">
            {I18N_LONG}
          </Tag>
          <Tag data-testid="g-stress-tag-token" tone="success">
            {LONG_TOKEN}
          </Tag>
        </div>
      ),
    },
  ],
}

const inputStress: GalleryStory = {
  id: 'stress-input',
  title: 'Stress — Input / Select',
  cases: [
    {
      key: 'value',
      label: 'Long value / placeholder',
      render: () => (
        <div className="flex flex-col gap-2 w-48">
          <Input
            data-testid="g-stress-input-value"
            aria-label="Long value"
            defaultValue={LONG_TOKEN}
          />
          <Input
            data-testid="g-stress-input-ph"
            aria-label="Long placeholder"
            placeholder={LONG_PROSE}
          />
          <Select
            data-testid="g-stress-select"
            aria-label="Long option"
            defaultValue="x"
            options={[{ value: 'x', label: I18N_LONG }]}
          />
        </div>
      ),
    },
  ],
}

const cardStress: GalleryStory = {
  id: 'stress-card',
  title: 'Stress — Card',
  note: 'long title + body; empty; loading',
  cases: [
    {
      key: 'long',
      label: 'Long title + body',
      render: () => (
        <Card
          data-testid="g-stress-card-long"
          title={I18N_LONG}
          className="w-56"
        >
          <p className="text-sm text-muted-foreground">{LONG_PROSE}</p>
        </Card>
      ),
    },
    {
      key: 'empty',
      label: 'Empty body',
      render: () => (
        <Card data-testid="g-stress-card-empty" title="Empty" className="w-56">
          {null}
        </Card>
      ),
    },
    {
      key: 'loading',
      label: 'Loading',
      render: () => (
        <Card
          data-testid="g-stress-card-loading"
          title="Loading"
          loading
          className="w-56"
        >
          <p>hidden by skeleton</p>
        </Card>
      ),
    },
  ],
}

const alertStress: GalleryStory = {
  id: 'stress-alert',
  title: 'Stress — Alert',
  cases: [
    {
      key: 'long',
      label: 'Long title + description',
      render: () => (
        <div className="w-64">
          <Alert
            data-testid="g-stress-alert"
            tone="warning"
            title={I18N_LONG}
            description={LONG_PROSE}
            closeLabel="Dismiss"
            onClose={() => undefined}
          />
        </div>
      ),
    },
  ],
}

const menuStress: GalleryStory = {
  id: 'stress-menu',
  title: 'Stress — Menu',
  cases: [
    {
      key: 'long',
      label: 'Long item labels',
      render: () => (
        <div className="w-48 rounded-md border border-border">
          <Menu
            data-testid="g-stress-menu"
            aria-label="Long menu"
            selectedKey="a"
            items={[
              { key: 'a', label: I18N_LONG },
              { key: 'b', label: 'Short' },
              { key: 'c', label: LONG_TOKEN },
            ]}
          />
        </div>
      ),
    },
  ],
}

interface SRow {
  id: string
  name: string
  note: string
}
const sCols: TableColumn<SRow>[] = [
  { key: 'name', title: 'Name', dataIndex: 'name' },
  { key: 'note', title: 'Note', dataIndex: 'note' },
]

const tableStress: GalleryStory = {
  id: 'stress-table',
  title: 'Stress — Table',
  note: 'long cell content; empty; loading',
  cases: [
    {
      key: 'long',
      label: 'Long cells',
      render: () => (
        <div className="w-72">
          <Table
            data-testid="g-stress-table-long"
            columns={sCols}
            dataSource={[
              { id: '1', name: I18N_LONG, note: LONG_TOKEN },
              { id: '2', name: 'Short', note: LONG_PROSE },
            ]}
            rowKey="id"
          />
        </div>
      ),
    },
    {
      key: 'empty',
      label: 'Empty',
      render: () => (
        <div className="w-72">
          <Table
            data-testid="g-stress-table-empty"
            columns={sCols}
            dataSource={[]}
            rowKey="id"
          />
        </div>
      ),
    },
    {
      key: 'loading',
      label: 'Loading',
      render: () => (
        <div className="w-72">
          <Table
            data-testid="g-stress-table-loading"
            columns={sCols}
            dataSource={[]}
            loading
            rowKey="id"
          />
        </div>
      ),
    },
  ],
}

const descStress: GalleryStory = {
  id: 'stress-descriptions',
  title: 'Stress — Descriptions',
  cases: [
    {
      key: 'long',
      label: 'Long values',
      render: () => (
        <div className="w-72">
          <Descriptions
            data-testid="g-stress-desc"
            bordered
            column={1}
            items={[
              { key: 'a', label: 'URL', children: LONG_TOKEN },
              { key: 'b', label: 'Name', children: I18N_LONG },
            ]}
          />
        </div>
      ),
    },
  ],
}

const statStress: GalleryStory = {
  id: 'stress-statistic',
  title: 'Stress — Statistic / List',
  cases: [
    {
      key: 'huge',
      label: 'Huge number / empty list',
      render: () => (
        <div className="flex flex-wrap gap-6 items-start">
          <Statistic
            data-testid="g-stress-stat"
            title="Total"
            value={HUGE_NUMBER}
            groupSeparator
          />
          <div className="w-48 rounded-md border border-border">
            <List
              data-testid="g-stress-list-empty"
              dataSource={[] as SRow[]}
              rowKey="id"
              empty="No items"
              renderItem={i => <span>{i.name}</span>}
            />
          </div>
        </div>
      ),
    },
  ],
}

export const stressStories: GalleryStory[] = [
  buttonStress,
  tagStress,
  inputStress,
  cardStress,
  alertStress,
  menuStress,
  tableStress,
  descStress,
  statStress,
]
