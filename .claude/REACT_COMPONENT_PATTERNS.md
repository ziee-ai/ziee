# React Component Patterns

This document describes the correct patterns for writing React components in the Ziee Chat application, particularly regarding store integration and data loading.

## Permission Gating

Every admin surface (settings page, sidebar entry, action button, form
field) MUST gate visibility on the current user's permissions. The
backend enforces authorization via `RequirePermissions<…>` extractors;
the UI exists to never show a button that will 403.

Three layers, in order of preference:

1. **Slot field** — for slot-registered surfaces (settings pages,
   sidebar entries, hub tabs). Add `permission: '...'` to the slot
   entry in `module.tsx`. The slot consumer handles menu filtering +
   inline 403 for deep links.
2. **`<Can>` wrapper** — for per-button gates. Renders `null` when
   denied.
   ```tsx
   <Can permission="users::delete">
     <Button danger onClick={handleDelete}>Delete</Button>
   </Can>
   ```
3. **`usePermission` hook** — for conditional logic with multiple
   branches, `disabled` props on form fields, conditionally building
   action arrays.
   ```tsx
   const canEdit = usePermission('users::edit')
   return <Form disabled={!canEdit}>…</Form>
   ```

Permission expressions support composition: `{ allOf: […] }`,
`{ anyOf: […] }`, nested.

**See [.claude/PERMISSION_GATING.md](./PERMISSION_GATING.md) for the
full pattern, semantics (`is_admin` vs Administrators group,
wildcards), anti-patterns, and the checklist for adding a new
feature.**

## Store Access Pattern

### ✅ CORRECT: Declarative Store Access

Components should **declaratively access store state** and let the store's initialization system handle data loading automatically.

```tsx
import { Stores } from '@/core/stores'

export function MyComponent() {
  // ✅ CORRECT: Just access the store state
  // The store proxy automatically triggers __init__ hooks when accessed
  const { items, loading } = Stores.MyStore

  const handleCreate = () => {
    Stores.MyStore.createItem({ name: 'New Item' })
  }

  return (
    <div>
      {loading && <Spinner />}
      {items.map(item => <ItemCard key={item.id} item={item} />)}
      <Button onClick={handleCreate}>Create</Button>
    </div>
  )
}
```

**Why this works:**
- The store proxy intercepts property access (e.g., `Stores.MyStore.items`)
- On first access, it automatically calls `__init__` hooks defined in the store
- The `__init__` hooks handle data loading and event subscription
- Components remain simple and declarative

### ❌ ANTI-PATTERN: Manual Data Loading in useEffect

**DO NOT** manually trigger store loading in `useEffect` hooks:

```tsx
// ❌ WRONG: Manual loading in useEffect
export function MyComponent() {
  const { items, loading, isInitialized } = Stores.MyStore

  // ❌ ANTI-PATTERN: Manually calling load methods
  useEffect(() => {
    if (!isInitialized) {
      Stores.MyStore.loadItems()  // ❌ DON'T DO THIS
    }
  }, [isInitialized])

  return <div>{/* ... */}</div>
}
```

**Problems with this approach:**
1. **Bypasses the meta-framework** - Ignores the built-in initialization system
2. **Tight coupling** - Component knows about loading implementation details
3. **Duplication** - Every component must implement the same loading logic
4. **Race conditions** - Multiple components might trigger loading simultaneously
5. **Testing complexity** - Harder to test and mock
6. **Inconsistent behavior** - Different components might load data differently

## Store Proxy Contract

`Stores.X` is a `Proxy` wrapping a Zustand store. Its `get` handler
dispatches on **what kind of property** is being read, in this order:

1. **Special props** (`__refTracker`, `__refCount`, `__destroyed`,
   `__init__`, `__destroy__`) — bookkeeping for the proxy itself.
   Safe to read anywhere. The framework uses these; you usually
   don't.

2. **Functions / actions** — `Stores.X.createFoo(...)`,
   `Stores.X.refreshBar()`. Returned directly without subscribing to
   any state. **Safe to call from anywhere**: components, event
   handlers, store-to-store calls, `__init__` / `__destroy__` hooks,
   plain modules.

3. **Nested store proxies** — properties whose value is itself a
   store proxy (detected via `__refTracker`). Returned as-is so
   reactivity stays local to that nested store. **Safe to read
   anywhere**, then access state on the nested store under the same
   rules.

4. **State values** — primitives, objects, arrays held in the
   store's state. **Reactive reads via the `useStore` hook** — only
   safe inside a React component or another React hook.

The proxy can't know syntactically whether a destructure like
`const { items } = Stores.X` happens at a component's top level or
in an event handler — both look identical to the trap. The contract
the codebase commits to is:

> **State values may only be read from inside a React component
> body or a React hook. Actions and nested stores may be called
> from anywhere.**

### ✅ CORRECT usage

```tsx
function MyList() {
  // Reactive read inside a component body — OK.
  const { items, loading } = Stores.Items

  const handleAdd = () => {
    // Action call inside an event handler — OK.
    Stores.Items.createItem({ name: 'New' })
  }

  return loading ? <Spin /> : items.map(/* ... */)
}

// Store-to-store action call from outside any component — OK.
export const useItemsStore = create((set, get) => ({
  __init__: {
    items: async () => {
      // Calling another store's action — fine, it's an action call.
      await Stores.Auth.refreshSession()
      const items = await ApiClient.Items.list()
      set({ items })
    },
  },
  // ...
}))
```

### ❌ ANTI-PATTERN: reactive reads outside a component

```tsx
// ❌ WRONG: top-level destructure of state in a non-component module.
//    Looks identical to the correct in-component form, but the
//    proxy will call `useStore(...)` outside a React render and
//    throw "Invalid hook call".
const { items } = Stores.Items
```

```tsx
// ❌ WRONG: state read inside an event handler.
function Toolbar() {
  const handleClick = () => {
    // `Stores.Items.items` enters the state-value branch → useStore
    // hook is called outside render → "Invalid hook call".
    console.log(Stores.Items.items.length)
  }
  return <Button onClick={handleClick}>Log</Button>
}

// ✅ Either snapshot at render time and close over it…
function Toolbar() {
  const { items } = Stores.Items
  const handleClick = () => console.log(items.length)
  return <Button onClick={handleClick}>Log</Button>
}

// …or read the raw state outside the hook system via the underlying
// `useStore.getState()` if you genuinely need a non-reactive read.
```

### Future safety net

A `scripts/lint-stores.ts` walker (Cluster G follow-up) will flag
reactive `Stores.X.<state-prop>` reads from outside React component
bodies / `use*` hooks at CI time, and a dev-only runtime guard in
`createStoreProxy` will throw a precise error when the state-value
branch is entered without an active React render. Neither is in place
yet — adhere to the contract by convention until they land.

## API Client Pattern

### ⚠️ CRITICAL: ApiClient is Auto-Generated

**NEVER edit `ApiClient` or `types.ts` manually!**

**Location:** `ui/src/api-client/`

**Auto-Generation Flow:**
```
Backend (Rust with OpenAPI attributes)
    ↓
cargo run -- --generate-openapi
    ↓
Generates openapi.json
    ↓
npm run generate-openapi (runs generate-endpoints.ts)
    ↓
Reads openapi.json → Generates types.ts
    ↓
index.ts dynamically builds ApiClient from types.ts
```

**What Gets Generated:**
- ✅ `types.ts` - All TypeScript interfaces, enums, request/response types
- ✅ `ApiClient` - Dynamically created from endpoint definitions
- ✅ Type-safe method calls with proper parameter and return types

**How to Add New Endpoints:**

```rust
// ❌ WRONG: Editing ui/src/api-client/index.ts manually
export const ApiClient = {
  Hub: {
    createFromHub: async () => { ... }  // DON'T DO THIS
  }
}

// ✅ CORRECT: Add endpoint in backend with OpenAPI attributes
#[api_v2_operation]
pub async fn create_assistant_from_hub(
    Json(request): Json<CreateAssistantFromHubRequest>,
) -> ApiResult<impl IntoApiResponse> {
    // Handler implementation
}

// Then run:
// 1. cargo run -- --generate-openapi
// 2. npm run generate-openapi
// 3. ApiClient.Hub.createAssistantFromHub() is now available!
```

**File Headers:**
```typescript
/**
 * ⚠️  DO NOT EDIT THIS FILE MANUALLY ⚠️
 * This file is automatically generated from the OpenAPI specification
 */
```

If you see this header, **NEVER edit the file!**

## Store Initialization Pattern

### How Store Initialization Works

The store proxy system automatically handles initialization through `__init__` hooks:

```tsx
// Store definition
export const useMyStore = create<MyState>((set, get) => ({
  items: [],
  loading: false,

  // Initialization hooks
  __init__: {
    // Called once when store is first accessed
    __store__: () => {
      console.log('Store initialized!')
      get().loadItems()

      // Subscribe to events
      const unsubscribe = Stores.EventBus.on('item.created', () => {
        get().loadItems()
      })

      set({ _eventUnsubscribers: [unsubscribe] })
    },

    // Called once when specific property is first accessed
    items: () => {
      console.log('Items property accessed!')
      get().loadItems()
    }
  },

  // Actions
  loadItems: async () => {
    set({ loading: true })
    const items = await ApiClient.Item.list()
    set({ items, loading: false })
  },
}))
```

**When you access `Stores.MyStore.items` in a component:**
1. Store proxy intercepts the access
2. Checks if `__init__.__store__` has been called → calls it if not
3. Checks if `__init__.items` has been called → calls it if not
4. Returns the actual `items` value
5. Component re-renders when `items` changes

## Error Handling Pattern

### ✅ CORRECT: Store-Level Error Handling

Handle errors in the store, expose error state to components:

```tsx
// Store
export const useMyStore = create<MyState>((set) => ({
  items: [],
  loading: false,
  error: null,

  loadItems: async () => {
    set({ loading: true, error: null })
    try {
      const items = await ApiClient.Item.list()
      set({ items, loading: false })
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to load items',
        loading: false
      })
    }
  },
}))

// Component
export function MyComponent() {
  const { message } = App.useApp()
  const { items, loading, error } = Stores.MyStore

  // ✅ CORRECT: React to error state changes
  useEffect(() => {
    if (error) {
      message.error(error)
    }
  }, [error, message])

  return <div>{/* ... */}</div>
}
```

### ❌ ANTI-PATTERN: Component-Level Error Handling for Loading

```tsx
// ❌ WRONG: Component handles loading errors
export function MyComponent() {
  const { message } = App.useApp()
  const { items, isInitialized } = Stores.MyStore

  useEffect(() => {
    if (!isInitialized) {
      Stores.MyStore.loadItems().catch((err) => {
        message.error('Failed to load items')  // ❌ Error handling in component
      })
    }
  }, [isInitialized, message])
}
```

## Component Responsibility

### Components Should:
- ✅ Access store state declaratively
- ✅ Call store actions (create, update, delete)
- ✅ Render UI based on store state
- ✅ Handle UI-specific state (form inputs, modals, filters)
- ✅ React to error state changes (show error messages)

### Components Should NOT:
- ❌ Trigger initial data loading
- ❌ Know about store initialization details
- ❌ Manage loading states manually
- ❌ Subscribe to events directly
- ❌ Handle business logic

## Loading States Pattern

### ✅ CORRECT: Use Store Loading State

```tsx
export function MyComponent() {
  const { items, loading } = Stores.MyStore

  if (loading && items.length === 0) {
    return <Spinner />  // Initial load
  }

  return (
    <div>
      {loading && <Text type="secondary">Refreshing...</Text>}
      {items.map(item => <ItemCard key={item.id} item={item} />)}
    </div>
  )
}
```

### Common Patterns:

```tsx
// Pattern 1: Show spinner on initial load only
if (loading && items.length === 0) {
  return <Spinner />
}

// Pattern 2: Show inline loading indicator when refreshing
{loading && <Text type="secondary">Loading...</Text>}

// Pattern 3: Disable actions while loading
<Button disabled={loading} onClick={handleCreate}>
  Create Item
</Button>
```

## Migration Guide

### Removing Anti-Patterns

If you find code with the anti-pattern, follow these steps:

**Before (Anti-pattern):**
```tsx
import { useEffect, useState } from 'react'

export function MyComponent() {
  const { items, loading, isInitialized } = Stores.MyStore

  useEffect(() => {
    if (!isInitialized) {
      Stores.MyStore.loadItems().catch(console.error)
    }
  }, [isInitialized])

  return <div>{/* ... */}</div>
}
```

**After (Correct pattern):**
```tsx
import { useState } from 'react'  // Remove useEffect import if not needed elsewhere

export function MyComponent() {
  const { items, loading } = Stores.MyStore  // Remove isInitialized

  // Remove the useEffect entirely

  return <div>{/* ... */}</div>
}
```

**Steps:**
1. Remove `useEffect` import if not used elsewhere
2. Remove `isInitialized` from store destructuring
3. Delete the entire `useEffect` block that calls `load*` methods
4. Ensure the store has proper `__init__` hooks defined

## Common UI Layout Components

### HeaderBarContainer

**IMPORTANT:** Use `HeaderBarContainer` for page header/title bars.

**Location:** `@/modules/layouts/app-layout/components/HeaderBarContainer`

**Pattern:**
```tsx
import { HeaderBarContainer } from '@/modules/layouts/app-layout/components/HeaderBarContainer'
import { Typography } from 'antd'

export function MyPage() {
  return (
    <div className="h-full flex flex-col overflow-hidden">
      {/* Page Header */}
      <HeaderBarContainer>
        <div className="h-full flex items-center justify-between w-full">
          <Typography.Title level={4} className="!m-0 !leading-tight truncate">
            Page Title
          </Typography.Title>
          {/* Optional right-side actions */}
        </div>
      </HeaderBarContainer>

      {/* Page Content */}
      <div className="flex-1 overflow-auto">
        {/* Your content here */}
      </div>
    </div>
  )
}
```

**Features:**
- Automatically adjusts padding based on sidebar collapsed state
- Consistent height (50px) and border styling
- Responsive to theme changes

**Reference:** See `SettingsPage.tsx` and `HubPage.tsx` for examples.

### Drawer Component

**CRITICAL:** Always use the custom `Drawer` component instead of Ant Design's `Drawer` directly.

**Location:** `@/modules/layouts/app-layout/components/Drawer`

**Why Use Custom Drawer:**
- ❌ **NEVER** import `Drawer` from `antd`
- ✅ **ALWAYS** import from `@/modules/layouts/app-layout/components/Drawer`

**Pattern:**
```tsx
// ✅ CORRECT: Import custom Drawer
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { Flex, Tag, Typography, Card } from 'antd'

const { Title, Text } = Typography

export function MyDetailsDrawer({ item, open, onClose }: MyDetailsDrawerProps) {
  if (!item) return null

  return (
    <Drawer
      title={item.name}
      open={open}
      onClose={onClose}
    >
      <Flex vertical className="gap-4">
        <div>
          <Title level={5}>Description</Title>
          <Text>{item.description}</Text>
        </div>
      </Flex>
    </Drawer>
  )
}
```

```tsx
// ❌ WRONG: Importing Drawer from antd
import { Drawer, Flex, Tag } from 'antd'  // DON'T DO THIS
```

**Features of Custom Drawer:**
- **ResizeHandle** - Left edge resize functionality
- **DivScrollY** - Proper scroll management
- **Custom Header** - Back button with IoIosArrowBack icon
- **Responsive Width** - 100% on mobile, configurable on desktop (default 520px)
- **Theme Integration** - Uses theme tokens for consistent styling
- **Array Footer Support** - Can pass array of footer elements

**Common Usage with Local State:**
```tsx
import { useState } from 'react'

export function MyCard({ item }: MyCardProps) {
  const [showDetails, setShowDetails] = useState(false)

  return (
    <>
      <Card hoverable onClick={() => setShowDetails(true)}>
        {/* Card content */}
      </Card>

      <MyDetailsDrawer
        item={showDetails ? item : null}
        open={showDetails}
        onClose={() => setShowDetails(false)}
      />
    </>
  )
}
```

**Reference:** See `ModelDetailsDrawer.tsx`, `McpServerDetailsDrawer.tsx`, and `AssistantDetailsDrawer.tsx` in hub modules.

## Form input descriptions: NO tooltips, always visible

For every antd `<Form.Item>` (settings forms, edit drawers, anywhere
except the chat composer):

- **Never** use `tooltip="..."` on the label. The question-mark hover
  icon hides the instruction from users who don't think to mouse-hover.
- **Always** put the requirement, example, format, valid range, or any
  other instruction directly under the input as `extra="..."` (or a
  custom `<Text type="secondary">` block) so it's in plain sight.
- This applies to inputs WITH specific requirements (formats, ranges,
  placeholders, semantic implications). A label like "Name" with a free
  text input doesn't need extra; "memory.max" with a "≥ 16 MiB" rule does.

```tsx
// ❌ WRONG — instruction is invisible until hover
<Form.Item
  name="memory_max_mib"
  label="memory.max"
  tooltip="cgroup v2 memory cap (MiB). OOM-kills the workload if exceeded."
>
  <InputNumber min={16} suffix="MiB" />
</Form.Item>

// ✅ RIGHT — instruction visible below input
<Form.Item
  name="memory_max_mib"
  label="memory.max"
  extra="cgroup v2 memory cap. OOM-kills the workload if exceeded. Minimum 16 MiB."
>
  <InputNumber min={16} suffix="MiB" />
</Form.Item>
```

**The one exception:** `ChatInput` (the chat composer). Inline tooltips
on its tiny toolbar buttons are fine — those aren't form inputs in the
settings-form sense.

## Summary

**Golden Rule:** Components access store state, stores handle initialization and loading.

- **Components** = View layer (declarative, reactive)
- **Stores** = Data layer (initialization, loading, caching, events)

This separation ensures:
- Consistent behavior across all components
- Single source of truth for loading logic
- Easier testing and maintenance
- Proper integration with the meta-framework
- Clean, simple components
