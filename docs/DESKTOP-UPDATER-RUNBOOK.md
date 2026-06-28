# Desktop Auto-Updater — Runbook

How the Ziee desktop app ships signed auto-updates, and the one-time setup an
operator must do. The code is wired end-to-end; the items below are the parts
that require secrets / GitHub settings and therefore can't live in the repo.

## How it works

- The app bundles `tauri-plugin-updater` and is configured
  (`src-app/desktop/tauri/tauri.conf.json`) to check a **static manifest** at:

  ```
  https://phibya.github.io/ziee-chat-new/latest.json
  ```

- On launch the app silently checks that endpoint and, if a newer version
  exists, shows a notification. The user installs from **Settings → About**
  (or the tray's "Check for updates…"), which downloads the signed bundle,
  verifies its signature against the baked-in public key, and restarts.

- Releases are cut by pushing a `vX.Y.Z` **tag**. The
  `.github/workflows/desktop-release.yml` workflow builds + signs the app for
  macOS (arm64 + x86_64), Windows, and Linux, uploads the bundles to a GitHub
  Release, then publishes `latest.json` to the `gh-pages` branch (which GitHub
  Pages serves at the URL above).

## One-time setup

### 1. Generate the updater signing keypair

```bash
cd src-app/desktop/tauri
npx tauri signer generate -w ~/.tauri/ziee_updater.key   # prompts for a password
```

This writes the private key (`~/.tauri/ziee_updater.key`, **keep secret**) and
the public key (`~/.tauri/ziee_updater.key.pub`).

> The `plugins.updater.pubkey` value baked into `tauri.conf.json` is the public
> half of a throwaway **dev** keypair. Its **private** half is NOT committed:
> `src-app/desktop/tauri/.tauri-keys/` is gitignored and is absent from a fresh
> checkout, so you cannot reuse the dev private key — there is nothing to copy.
> Generate your own keypair with the command above (it writes to
> `~/.tauri/ziee_updater.key`), then replace the `plugins.updater.pubkey` value
> with your new public key and register the matching private key as the CI
> secret below. The public key in `tauri.conf.json` and the private key in CI
> MUST be a matching pair, or installs fail signature verification.

### 2. Add the repository secrets

GitHub → repo **Settings → Secrets and variables → Actions → New secret**:

| Secret | Value |
|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | full contents of the private key file |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | the password you set (empty string if none) |

`tauri-action` reads these automatically and emits a `.sig` next to every
updater bundle.

### 3. Enable GitHub Pages on the `gh-pages` branch

GitHub → repo **Settings → Pages** → Source = **Deploy from a branch** →
Branch = **`gh-pages`** / **`/ (root)`**. The first release workflow run
creates that branch; after that the manifest is live at
`https://phibya.github.io/ziee-chat-new/latest.json`.

## Cutting a release

1. Bump the version if you want it explicit in-repo (optional — CI overrides
   `tauri.conf.json`'s `version` from the tag at build time anyway):
   the source of truth at release time is the tag.
2. Tag and push:

   ```bash
   git tag v0.2.0
   git push origin v0.2.0
   ```

3. The `Desktop Release` workflow runs: builds + signs per-platform, creates the
   GitHub Release `v0.2.0`, and publishes `latest.json` to `gh-pages`.
4. Existing installs pick up the update on their next launch check.

## Notes & caveats

- **OS code-signing / notarization is NOT configured.** Updater *signature*
  verification works (auto-update is secure), but first-time installs show the
  OS "unidentified developer" warning on macOS/Windows. Add Apple notarization
  + Windows Authenticode later if desired.
- **Build database in CI.** The desktop build compiles the whole server, whose
  SQLx macros need a Postgres+pgvector at build time (`.cargo/config.toml`
  points `DATABASE_URL` at `:54321`). Linux uses a `pgvector/pgvector:pg18`
  service container; macOS/Windows provision Postgres natively in the workflow.
  The **macOS/Windows DB-setup steps should be validated on the first real tag
  push** — Linux is the reference path.
- **Manifest format.** `latest.json` is the Tauri static-manifest shape
  (`version`, `notes`, `pub_date`, `platforms`) keyed `darwin-aarch64`,
  `darwin-x86_64`, `linux-x86_64`, `windows-x86_64`. It is assembled by the
  shared `scripts/updater/build-latest-json.mjs` (used by both CI and tests).

## Testing (no release / no secrets needed)

```bash
just check-updater       # Tier 1 store + Tier 2 manifest + Tier 3 signing round-trip
just check-updater-ci    # Tier 4: runs the Pages workflow under `act` + actionlint
```

- **Tier 3** auto-generates an ephemeral key, signs a fixture with
  `tauri signer`, and verifies the manifest signature with `minisign-verify`
  (the same crate the runtime updater uses), asserting a tampered artifact
  fails.
- **Tier 4** runs `.github/workflows/desktop-updater-pages-test.yml` locally via
  `act` against a temp bare git repo (no GitHub), asserting the published
  `latest.json` is correct.
