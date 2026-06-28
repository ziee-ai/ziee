# tauri-driver E2E (Layer 3)

WebDriver-driven smoke against the **production-bundled** `Ziee.app`.

The default E2E suite (`tests/e2e/`) covers the SPA + real backend in dev
mode (Vite + `cargo run`). That's the primary coverage and what CI runs.

This layer exists to catch regressions that only show up after the
Tauri bundling pipeline — bad `productName`, broken `tauri.conf.json`
capability ACL, missing rust-embed assets, wrong `mainBinaryName`, etc.

## Prerequisites

1. **Build the bundle first.** From `src-app/`:
   ```bash
   cd desktop/tauri && cargo tauri build
   # → target/release/bundle/macos/Ziee.app
   ```

2. **Install `tauri-driver`.** From any directory:
   ```bash
   cargo install tauri-driver --locked
   ```

3. **Install the platform WebDriver.**
   - **macOS**: enable Safari's WebDriver:
     ```bash
     sudo safaridriver --enable
     ```
     (Tauri on macOS uses WebKit; `safaridriver` ships with macOS.)
   - **Linux**: `apt install webkit2gtk-driver`.
   - **Windows**: `Microsoft.WebDriver` (matching your Edge build).

4. **selenium-webdriver** is already in `desktop/ui`'s
   `devDependencies` — `npm install` covers it.

## Running

```bash
cd src-app/desktop/ui
npm run test:tauri-driver
```

The script:
1. Spawns `tauri-driver` on port 4444 (background).
2. Connects via `selenium-webdriver`'s WebDriver client.
3. Boots `Ziee.app` as a capability target.
4. Waits for the React root element to render.
5. Asserts the window title contains "Ziee".
6. Kills `tauri-driver`.

Exit 0 = pass. Anything else = fail.

## Why selenium-webdriver and not webdriverio / Playwright?

- **Playwright** doesn't speak the W3C WebDriver protocol that
  `tauri-driver` proxies — it uses CDP / its own protocol.
- **webdriverio** works but pulls in a much larger config surface.
  selenium-webdriver is one npm dep + ~40 lines of JS.

## Limitations

- macOS only: this harness currently targets the macOS bundle —
  `smoke.mjs` resolves `target/release/bundle/macos/Ziee.app`, so it
  does not run on Linux/Windows as-is. (On macOS no DISPLAY is needed
  for `safaridriver`; a Linux port would need X11/Wayland for
  `webkit2gtk-driver` plus the corresponding Linux bundle path.)
- No isolation: uses the user's real macOS data dir
  (`~/Library/Application Support/com.ziee.chat/`). Don't run while
  you have a real session open (port collision).
- One-shot: the script boots, asserts, kills. No interactive REPL.
  For deeper exploration of the bundled app, use Safari's Web
  Inspector against the running `Ziee.app` directly.
