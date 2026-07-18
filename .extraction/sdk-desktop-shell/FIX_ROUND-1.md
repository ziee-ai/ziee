# Chunk `sdk-desktop-shell` — FIX round 1

The blind audit (LEDGER, 10 angles incl. equivalence + security + the
platform-override proof) surfaced no behavioural regression. No mechanical
issues required fixing during implementation — the `@ziee/shell` package tsconfig
(paths for `@ziee/{kit,framework}`, `env.d.ts`, CSS side-effect) and the ziee
`ui/`+`desktop/ui` `@ziee/shell` path mappings were already in place from the
`sdk-shell` chunk, so the new `layouts/AppLayout`, `layouts/appLayoutSlots`,
`hooks/useWindowMinSize`, and `settings/SettingsPageContainer` sub-paths resolved
first time. shell tsc=0, ui tsc=0, desktop tsc=0 on the first full pass.

Three DELIBERATE, non-regression transforms (declared T-1/T-2/T-4, not fixes):
- the 2 platform-variant leaves became injected props (app injects via `@/`, so
  the desktop `.desktop` swap fires at ziee's site — remediation (a));
- the app-store reads became typed local casts on the shell's own `Stores`
  (runtime read byte-identical, same live proxy);
- the min-size hooks split (pure → shell; the store-coupled `useMainContentMinSize`
  stayed app-side composing shell's exported helpers).

Scoped deviations from the B-1 brief, both declared as resolved Decisions (D-3,
D-4), NOT silent drops: `SettingsPage`(body)+`SettingsLayout` and
`Drawer`+`HeaderBarContainer` stay app-side because they are not generic and/or
their `@/` desktop-swap must be preserved for 40+ consumers — moving them would
CAUSE the desktop regression this chunk exists to prevent. Only the genuinely
generic pieces (AppLayout structure, the min-size hooks, the slot types,
SettingsPageContainer) moved.

Housekeeping: the verification run flipped `sdk/packages/gallery/scripts/cli.mjs`'s
file mode (100644→100755) and overwrote ziee's committed `RUNTIME_FINDINGS.md`;
both reverted via `git checkout` (verified clean). `RUNTIME_FINDINGS.jsonl` is
gitignored (generated).

**New confirmed findings:** 0
