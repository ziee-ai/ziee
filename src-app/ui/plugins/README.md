# Vite Plugins

Custom Vite plugins for code quality and development tools.

## Plugins

### 1. `babel-plugin-add-component-name.cjs`

**Purpose:** Adds `data-component-name` attributes to React components in dev/test modes.

**When it runs:**
- Development mode
- Test mode

**What it does:**
```tsx
// Before (source code)
export function UserCard() {
  return <div>...</div>
}

// After (in browser)
<div data-component-name="UserCard">...</div>
```

**Usage in tests:**
```typescript
import { getComponent } from '@/tests/helpers/component-selectors'

const userCard = getComponent(page, 'UserCard') // Full IntelliSense!
```

---

### 2. `vite-plugin-component-types.js`

**Purpose:** Auto-generates TypeScript types for all React component names.

**When it runs:**
- Plugin initialization
- File changes (debounced 500ms)
- Build start

**What it generates:**
```typescript
// tests/helpers/component-names.generated.ts
export type ComponentName =
  | 'UserCard'
  | 'UserSettings'
  | 'EditUserDrawer'
  // ... all component names
```

**Features:**
- ✅ Detects duplicate component names
- ✅ Auto-regenerates on file changes
- ✅ Provides IntelliSense in tests

**Output:**
```bash
[component-types] Generated types for 66 components

# If duplicates found:
[component-types] ⚠️  Found 2 duplicate component name(s):
  • "UserCard" defined in:
    - modules/user/UserCard.tsx
    - modules/admin/UserCard.tsx
```

---

### 3. `vite-plugin-form-names.js`

**Purpose:** Detects duplicate Ant Design `<Form name="...">` values.

**Why this matters:**
- Ant Design generates input IDs as: `{formName}_{fieldName}`
- Without unique form names → duplicate IDs in DOM
- Duplicate IDs break:
  - Accessibility (screen readers)
  - Form labels (`<label for="...">`)
  - E2E tests (ambiguous selectors)

**When it runs:**
- Plugin initialization
- File changes (debounced 1000ms)
- Build start

**What it checks:**
```tsx
// ✅ Good - unique names
<Form name="login-form">...</Form>
<Form name="signup-form">...</Form>

// ❌ Bad - duplicate names
<Form name="user-form">...</Form>  // In UserEdit.tsx
<Form name="user-form">...</Form>  // In AdminUserEdit.tsx
```

**Output:**
```bash
# All unique
[form-names] ✓ All form names are unique (15 forms found)

# Duplicates found
[form-names] ⚠️  Found 1 duplicate form name(s):
  • "user-form" defined in:
    - modules/user/EditUserForm.tsx
    - modules/admin/AdminUserForm.tsx

[form-names] Duplicate names will cause ID collisions in the DOM.
[form-names] Ant Design generates IDs as: {formName}_{fieldName}
```

**To fail build on duplicates:**
Uncomment this line in the plugin:
```javascript
throw new Error(`Duplicate form names found: ${duplicates.map(d => d.name).join(', ')}`)
```

---

## Plugin Configuration

### `vite.config.ts`

```typescript
import { componentTypesPlugin } from './plugins/vite-plugin-component-types.js'
import { formNamesPlugin } from './plugins/vite-plugin-form-names.js'

export default defineConfig({
  plugins: [
    react({
      babel: {
        plugins: [
          ...(isDev || isTest ? ['./plugins/babel-plugin-add-component-name.cjs'] : []),
        ],
      },
    }),
    componentTypesPlugin({
      srcDir: 'src',
      outputFile: 'tests/helpers/component-names.generated.ts',
    }),
    formNamesPlugin({
      srcDir: 'src',
    }),
  ],
})
```

---

## Best Practices

### Component Names
- ✅ Use PascalCase: `UserCard`, `EditUserDrawer`
- ✅ Be descriptive: `EditUserDrawer` not `EditDrawer`
- ❌ Avoid duplicates across modules

### Form Names
- ✅ Use kebab-case: `login-form`, `edit-user-form`
- ✅ Include context: `edit-user-form` not just `form`
- ✅ Make unique across entire app
- ❌ Don't reuse names in different components

**Example naming:**
```tsx
// ✅ Good - descriptive and unique
<Form name="user-profile-edit-form">
<Form name="admin-create-user-form">
<Form name="mcp-server-config-form">

// ❌ Bad - too generic, likely to duplicate
<Form name="edit-form">
<Form name="user-form">
<Form name="form">
```

---

## Troubleshooting

### Plugin not running?
- Check `vite.config.ts` includes the plugin
- Restart dev server
- Clear cache: `rm -rf node_modules/.vite`

### Duplicates not detected?
- Check regex patterns in plugin
- Verify file is in `src/` directory
- Check file has `.tsx` extension

### False positives?
- Verify the duplicate is actually in multiple files
- Check relative paths in error message
- Open files to confirm

---

## See Also

- [Ant Design Form Documentation](https://ant.design/components/form)
- [Vite Plugin API](https://vitejs.dev/guide/api-plugin.html)
- [Babel Plugin Handbook](https://github.com/jamiebuilds/babel-handbook)
