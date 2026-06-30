/**
 * Stories for the interactive form controls. Each exported `GalleryStory` is one
 * component's section. Cases favor uncontrolled (`defaultValue`/`defaultChecked`)
 * or no-op handlers so the canvas stays static + deterministic for snapshots.
 */
import { Search } from 'lucide-react'
import {
  Button,
  Checkbox,
  Combobox,
  DatePicker,
  Input,
  InputNumber,
  MultiSelect,
  PasswordInput,
  RadioGroup,
  Segmented,
  Select,
  Switch,
  Textarea,
} from '@/components/ui'
import type { GalleryStory } from '../story'

const noop = () => undefined

const sizes = ['sm', 'default', 'lg'] as const

const opts = [
  { value: 'a', label: 'Option A' },
  { value: 'b', label: 'Option B' },
  { value: 'c', label: 'Option C' },
]

const btnVariants = [
  'default',
  'secondary',
  'outline',
  'ghost',
  'destructive',
  'link',
] as const

const buttonStory: GalleryStory = {
  id: 'button',
  title: 'Button',
  note: 'full variant × size matrix + per-variant states; icon-only, block',
  cases: [
    // Full variant × size cross-product (the audit flagged the old story only
    // showed variants-at-default-size and sizes-at-default-variant).
    ...btnVariants.map(v => ({
      key: `variant-${v}`,
      label: v,
      render: () => (
        <div className="flex flex-wrap items-center gap-2">
          {sizes.map(s => (
            <Button
              key={s}
              data-testid={`g-btn-${v}-${s}`}
              variant={v}
              size={s}
            >
              {s}
            </Button>
          ))}
          <Button
            data-testid={`g-btn-${v}-disabled`}
            variant={v}
            disabled
          >
            disabled
          </Button>
          <Button data-testid={`g-btn-${v}-loading`} variant={v} loading>
            loading
          </Button>
        </div>
      ),
    })),
    {
      key: 'icon',
      label: 'Icon / icon-only / block',
      render: () => (
        <div className="flex flex-wrap items-center gap-2">
          <Button data-testid="g-btn-icon" icon={<Search />}>
            With icon
          </Button>
          <Button
            data-testid="g-btn-icononly"
            size="icon"
            tooltip="Search"
            icon={<Search />}
          />
          <div className="w-48">
            <Button data-testid="g-btn-block" block>
              Block
            </Button>
          </div>
        </div>
      ),
    },
  ],
}

const inputStory: GalleryStory = {
  id: 'input',
  title: 'Input',
  note: 'default / filled / invalid / disabled / prefix-suffix / sizes',
  cases: [
    {
      key: 'basic',
      label: 'Basic',
      render: () => (
        <div className="flex flex-col gap-2 w-56">
          <Input
            data-testid="g-input-default"
            aria-label="Default input"
            placeholder="Placeholder"
          />
          <Input
            data-testid="g-input-filled"
            aria-label="Filled input"
            defaultValue="Filled value"
          />
          <Input
            data-testid="g-input-invalid"
            aria-label="Invalid input"
            invalid
            defaultValue="Bad"
          />
          <Input
            data-testid="g-input-disabled"
            aria-label="Disabled input"
            disabled
            defaultValue="Off"
          />
        </div>
      ),
    },
    {
      key: 'affix',
      label: 'Prefix / suffix / clear',
      render: () => (
        <div className="flex flex-col gap-2 w-56">
          <Input
            data-testid="g-input-prefix"
            aria-label="Search input"
            prefix={<Search />}
            placeholder="Search"
          />
          <Input
            data-testid="g-input-clear"
            aria-label="Clearable input"
            allowClear
            defaultValue="Clearable"
          />
        </div>
      ),
    },
    {
      key: 'sizes',
      label: 'Sizes',
      render: () => (
        <div className="flex flex-col gap-2 w-56">
          {sizes.map(s => (
            <Input
              key={s}
              data-testid={`g-input-size-${s}`}
              aria-label={`Input size ${s}`}
              size={s}
              placeholder={s}
            />
          ))}
        </div>
      ),
    },
  ],
}

const passwordStory: GalleryStory = {
  id: 'password-input',
  title: 'PasswordInput',
  cases: [
    {
      key: 'default',
      label: 'Default',
      render: () => (
        <div className="w-56">
          <PasswordInput
            data-testid="g-pwd-default"
            aria-label="Password"
            showLabel="Show password"
            hideLabel="Hide password"
            defaultValue="secret123"
          />
        </div>
      ),
    },
  ],
}

const textareaStory: GalleryStory = {
  id: 'textarea',
  title: 'Textarea',
  cases: [
    {
      key: 'states',
      label: 'Default / invalid / disabled',
      render: () => (
        <div className="flex flex-col gap-2 w-72">
          <Textarea
            data-testid="g-ta-default"
            aria-label="Default textarea"
            placeholder="Type here…"
          />
          <Textarea
            data-testid="g-ta-invalid"
            aria-label="Invalid textarea"
            invalid
            defaultValue="Bad"
          />
          <Textarea
            data-testid="g-ta-disabled"
            aria-label="Disabled textarea"
            disabled
            defaultValue="Off"
          />
        </div>
      ),
    },
  ],
}

const inputNumberStory: GalleryStory = {
  id: 'input-number',
  title: 'InputNumber',
  cases: [
    {
      key: 'sizes',
      label: 'Sizes',
      render: () => (
        <div className="flex flex-col gap-2 w-40">
          {sizes.map(s => (
            <InputNumber
              key={s}
              data-testid={`g-num-${s}`}
              aria-label={`Number ${s}`}
              size={s}
              defaultValue={42}
              suffix="px"
            />
          ))}
          <InputNumber
            data-testid="g-num-invalid"
            aria-label="Invalid number"
            invalid
            defaultValue={7}
          />
        </div>
      ),
    },
  ],
}

const selectStory: GalleryStory = {
  id: 'select',
  title: 'Select',
  note: 'default / sizes / invalid / disabled / clearable',
  cases: [
    {
      key: 'sizes',
      label: 'Sizes',
      render: () => (
        <div className="flex flex-col gap-2 w-56">
          {sizes.map(s => (
            <Select
              key={s}
              data-testid={`g-sel-${s}`}
              aria-label={`Select ${s}`}
              size={s}
              placeholder="Choose…"
              options={opts}
            />
          ))}
        </div>
      ),
    },
    {
      key: 'states',
      label: 'Filled / invalid / disabled / clearable',
      render: () => (
        <div className="flex flex-col gap-2 w-56">
          <Select
            data-testid="g-sel-filled"
            aria-label="Filled"
            defaultValue="a"
            options={opts}
          />
          <Select
            data-testid="g-sel-invalid"
            aria-label="Invalid"
            invalid
            defaultValue="b"
            options={opts}
          />
          <Select
            data-testid="g-sel-disabled"
            aria-label="Disabled"
            disabled
            defaultValue="c"
            options={opts}
          />
          <Select
            data-testid="g-sel-clear"
            aria-label="Clearable"
            allowClear
            clearLabel="Clear"
            defaultValue="a"
            options={opts}
          />
        </div>
      ),
    },
  ],
}

const comboboxStory: GalleryStory = {
  id: 'combobox',
  title: 'Combobox',
  cases: [
    {
      key: 'default',
      label: 'Default / disabled',
      render: () => (
        <div className="flex flex-col gap-2 w-56">
          <Combobox
            data-testid="g-cmb-default"
            aria-label="Combobox"
            placeholder="Pick one"
            searchPlaceholder="Search…"
            emptyText="No matches"
            options={opts}
          />
          <Combobox
            data-testid="g-cmb-disabled"
            aria-label="Combobox disabled"
            disabled
            defaultValue="a"
            placeholder="Pick one"
            searchPlaceholder="Search…"
            emptyText="No matches"
            options={opts}
          />
        </div>
      ),
    },
  ],
}

const multiSelectStory: GalleryStory = {
  id: 'multi-select',
  title: 'MultiSelect',
  cases: [
    {
      key: 'default',
      label: 'Empty / filled',
      render: () => (
        <div className="flex flex-col gap-2 w-64">
          <MultiSelect
            data-testid="g-ms-empty"
            aria-label="MultiSelect"
            placeholder="Pick some"
            searchPlaceholder="Search…"
            emptyText="No matches"
            removeLabel={l => `Remove ${l}`}
            options={opts}
          />
          <MultiSelect
            data-testid="g-ms-filled"
            aria-label="MultiSelect filled"
            defaultValue={['a', 'b']}
            placeholder="Pick some"
            searchPlaceholder="Search…"
            emptyText="No matches"
            removeLabel={l => `Remove ${l}`}
            options={opts}
          />
        </div>
      ),
    },
  ],
}

const switchStory: GalleryStory = {
  id: 'switch',
  title: 'Switch',
  cases: [
    {
      key: 'states',
      label: 'Off / on / disabled / loading',
      render: () => (
        <>
          <Switch data-testid="g-sw-off" aria-label="Off" />
          <Switch data-testid="g-sw-on" aria-label="On" defaultChecked />
          <Switch data-testid="g-sw-disabled" aria-label="Disabled" disabled />
          <Switch
            data-testid="g-sw-loading"
            aria-label="Loading"
            loading
            defaultChecked
          />
          <Switch data-testid="g-sw-label" label="With label" />
        </>
      ),
    },
  ],
}

const checkboxStory: GalleryStory = {
  id: 'checkbox',
  title: 'Checkbox',
  cases: [
    {
      key: 'states',
      label: 'Unchecked / checked / indeterminate / disabled / invalid',
      render: () => (
        <div className="flex flex-col gap-2">
          <Checkbox data-testid="g-cb-off" label="Unchecked" />
          <Checkbox data-testid="g-cb-on" label="Checked" defaultChecked />
          <Checkbox
            data-testid="g-cb-ind"
            label="Indeterminate"
            indeterminate
          />
          <Checkbox data-testid="g-cb-disabled" label="Disabled" disabled />
          <Checkbox data-testid="g-cb-invalid" label="Invalid" invalid />
        </div>
      ),
    },
  ],
}

const radioStory: GalleryStory = {
  id: 'radio-group',
  title: 'RadioGroup',
  cases: [
    {
      key: 'orientations',
      label: 'Vertical / horizontal',
      render: () => (
        <div className="flex flex-col gap-4">
          <RadioGroup
            data-testid="g-radio-v"
            aria-label="Vertical"
            defaultValue="a"
            options={opts}
          />
          <RadioGroup
            data-testid="g-radio-h"
            aria-label="Horizontal"
            orientation="horizontal"
            defaultValue="b"
            options={opts}
          />
          <RadioGroup
            data-testid="g-radio-disabled"
            aria-label="Disabled"
            disabled
            defaultValue="a"
            options={opts}
          />
        </div>
      ),
    },
  ],
}

const segmentedStory: GalleryStory = {
  id: 'segmented',
  title: 'Segmented',
  cases: [
    {
      key: 'sizes',
      label: 'Sizes',
      render: () => (
        <div className="flex flex-col gap-2 items-start">
          {sizes.map(s => (
            <Segmented
              key={s}
              data-testid={`g-seg-${s}`}
              aria-label={`Segmented ${s}`}
              size={s}
              defaultValue="a"
              options={opts}
            />
          ))}
        </div>
      ),
    },
  ],
}

const datePickerStory: GalleryStory = {
  id: 'date-picker',
  title: 'DatePicker',
  cases: [
    {
      key: 'states',
      label: 'Empty / filled / disabled',
      render: () => (
        <div className="flex flex-col gap-2 w-56">
          <DatePicker
            data-testid="g-date-empty"
            aria-label="Date"
            placeholder="Pick a date"
            onChange={noop}
          />
          <DatePicker
            data-testid="g-date-filled"
            aria-label="Date filled"
            placeholder="Pick a date"
            value="2024-04-29"
            onChange={noop}
          />
          <DatePicker
            data-testid="g-date-disabled"
            aria-label="Date disabled"
            placeholder="Pick a date"
            disabled
            onChange={noop}
          />
        </div>
      ),
    },
  ],
}

export const controlStories: GalleryStory[] = [
  buttonStory,
  inputStory,
  passwordStory,
  textareaStory,
  inputNumberStory,
  selectStory,
  comboboxStory,
  multiSelectStory,
  switchStory,
  checkboxStory,
  radioStory,
  segmentedStory,
  datePickerStory,
]
