# Frontend Dependency Hygiene

How we keep `src-app/ui/` clean: dependency upgrades, antd deprecation
tracking, and the `npm run check` gate.

---

## The `npm run check` gate

`src-app/ui/package.json` script:

```json
"check": "npm run generate-openapi && tsc && npx --no-install antd doctor && npx --no-install antd lint src"
```

Four steps, fail-fast on each:

1. **`generate-openapi`** — regenerates `api-client/types.ts` from the
   backend's OpenAPI schema. Catches type drift between FE and BE.
2. **`tsc`** — full project type-check (includes `src/` and `tests/`).
3. **`antd doctor`** — 14 install / peer-deps / dup-package / theme /
   CSS-in-JS checks. Should be 14/14 green.
4. **`antd lint src`** — scans for deprecated APIs, accessibility gaps,
   and performance smells across all `src/` `.tsx` files.

CI (when it exists) and pre-push (`just check` if added) should both
run this.

---

## `@ant-design/cli` workflow

The CLI bundles antd API knowledge for versions 3–6 fully offline. We
use four commands:

| Command | Purpose | When to run |
|---|---|---|
| `npx antd doctor` | Install/config sanity | Part of `npm run check`; every PR |
| `npx antd lint src` | Find deprecated/a11y/perf in source | Part of `npm run check`; every PR |
| `npx antd usage src` | Import inventory | After antd minor bumps, to verify nothing surprising |
| `npx antd migrate <from> <to>` | Cross-major migration guide | Only when bumping antd major |

The `just antd-check` recipe at the repo root runs doctor + lint + usage
and stamps outputs into `src-app/ui/docs/antd-diagnostics/<date>/`. Use
this when you want a snapshot for comparison.

Baseline snapshots:
- `src-app/ui/docs/antd-diagnostics/2026-05-25-baseline/` — first run of
  `@ant-design/cli`, captured antd 6.0.0 state (66 deprecations).
  Reference for what the cleanup looked like at the start.

---

## Dependency bump cadence

Two tiers:

### Within-major bumps (quarterly, low-risk)

Run `npm outdated` in `src-app/ui/` and bump everything where
**Wanted ≤ Latest within current major**. Process:

1. `cd src-app/ui && npm update` — bumps caret-range deps.
2. For deps pinned without caret (e.g. `@biomejs/biome` historically),
   explicitly `npm install --save-dev <pkg>@^<latest-within-major>`.
3. Run `npm run check` — fix any new tsc cascade, lint, or doctor errors.
4. Run `npm run test:e2e` — confirm no behavior regressions.
5. Commit as one `chore(ui/deps): npm update — ...` per logical group
   (runtime libs vs build chain vs types).

### Cross-major bumps (per-PR, case-by-case)

Each major version bump is its own evaluation:

- **Read the changelog and migration guide first.**
- Estimate scope: how many files touch the deprecated APIs.
- Land as its own focused PR. Don't batch multiple major bumps —
  cascading failures get hard to triage.
- After bumping, re-run `npx antd lint src` — antd minor versions add
  new deprecations, and major versions of other libs often surface them
  too.

Currently deferred (as of 2026-05-25):

| Dep | Current | Latest | Why deferred |
|---|---|---|---|
| `typescript` | 5.9.x | 6.0.3 | Breaking checker changes; needs a triage pass |
| `vite` | 6.4.x | 8.0.14 | 2 majors skipped; plugin compatibility matrix needed |
| `@vitejs/plugin-react` | 4.x | 6.x | Couple with vite bump |
| `i18next` + `react-i18next` | 25 + 15 | 26 + 17 | Bump together; review v6 API changes |
| `immer` | 10.x | 11.x | Map/Set draft semantics changed |
| `streamdown` | 1.x | 2.x | Markdown-rendering breaking changes; check chat impact |
| `@types/node` | 24.x | 25.x | Node-API drift |
| `bcryptjs`, `uuid` | 2.x, 11.x | 3.x, 14.x | Test-helper deps; defer |
| `xlsx` | 0.18.5 (vendor-locked) | n/a | Do NOT bump — upstream took it private |

---

## Working with antd deprecations

When `antd lint src` flags something, the message format is:

```
⚠ <file>:<line> [deprecated]
    <Component> `<prop>` is deprecated. <migration hint>
```

Common patterns we've seen and the fix shape:

| Pattern | Fix |
|---|---|
| `Alert message="X"` | `Alert title="X"` |
| `Alert closable onClose={X}` | `Alert closable={{ onClose: X }}` |
| `Spin tip="X"` | `Spin description="X"` |
| `Modal destroyOnClose` | `Modal destroyOnHidden` |
| `Card bordered={X}` | `Card variant={X ? 'outlined' : 'borderless'}` |
| `Divider type="vertical"` | `Divider orientation="vertical"` |
| `Dropdown overlayStyle={X}` / `overlayClassName="X"` | `Dropdown styles={{ root: X }}` / `classNames={{ root: 'X' }}` |
| `Dropdown dropdownRender={f}` | `Dropdown popupRender={f}` |
| `Select showSearch optionFilterProp="X"` | `Select showSearch={{ optionFilterProp: 'X' }}` |
| `Input addonBefore={X}` | `Input prefix={X}` (in-input variant) |
| `InputNumber addonAfter="MiB"` | `InputNumber suffix="MiB"` |
| `Space direction="vertical" size="X"` | `<Flex vertical gap="X" />` |
| `Drawer width={N}` (raw antd import) | Use project wrapper at `@/modules/layouts/app-layout/components/Drawer` with `size={N}` |

When in doubt: `npx antd info <Component>` shows current API + since-version markers.

---

## Anti-patterns to avoid

- Don't import `Drawer` directly from antd — use the project wrapper at
  `@/modules/layouts/app-layout/components/Drawer`. The wrapper handles
  mobile responsiveness (100% width on xs) and consistent styling.
- Don't suppress antd lint findings with comments. The `check` gate is
  the source of truth; if a finding is genuinely false-positive, file
  an upstream bug. Otherwise migrate.
- Don't run `npm audit fix --force` blindly. It can downgrade
  major-locked deps. Address vulnerabilities case-by-case.
- Don't bump multiple major versions in the same PR. Triage gets
  exponentially harder.
