/**
 * Posthoc fix for tauri-plugin-decorum's overlay titlebar on Windows.
 *
 * The plugin (enabled via `create_overlay_titlebar()` in
 * `desktop/tauri/src/modules/backend/mod.rs` — Windows-only `cfg`)
 * injects a fixed-positioned container at z-index 100 that owns the
 * top 32px:
 *
 *   <div data-tauri-decorum-tb
 *        style="top:0;left:0;z-index:100;width:100%;height:32px;
 *               display:flex;justify-content:end;...">
 *     <div data-tauri-drag-region style="width:100%;height:100%"></div>
 *     <button id="decorum-tb-minimize" class="decorum-tb-btn"></button>
 *     <button id="decorum-tb-maximize" class="decorum-tb-btn"></button>
 *     <button id="decorum-tb-close"    class="decorum-tb-btn"></button>
 *   </div>
 *
 * Our `SidebarToggleButton` sits at `top:0; z-index:10` — so the
 * decorum container (z:100) paints above it and the inner drag region
 * (width:100%) intercepts the click. Result on Windows: the chevron is
 * visible but unclickable for most of its hit area; only places the
 * drag region happens not to cover do anything. On macOS the plugin
 * doesn't inject (its `cfg(target_os = "windows")` gate skips the
 * `create_overlay_titlebar` call), so the bug is Windows-only.
 *
 * Fix shape:
 *   1. Make the outer decorum container `pointer-events: none`. The
 *      empty space in it becomes click-through; its children with
 *      `pointer-events: auto` (drag region + 3 buttons) keep working.
 *   2. Carve a 48px left margin on the drag region so it doesn't span
 *      the SidebarToggleButton's hit area. The 48 ≈ the button's 28px
 *      width + 12px left-margin + a few px breathing room.
 *
 * Implementation: inject a `<style>` once on first import. CSS-only
 * (no MutationObserver needed) — the selectors match whenever decorum
 * decides to mount its container. `!important` is needed because
 * decorum sets the drag region's `width: 100%` as an inline style.
 */

import { isTauriView, isWindows } from './platform'

let installed = false

export function installDecorumTitlebarFix(): void {
  if (installed) return
  installed = true

  // Only Windows + Tauri renders the decorum titlebar — guard the CSS
  // injection so a future macOS Tauri build (or the web preview) isn't
  // affected.
  if (!isTauriView || !isWindows) return

  const SIDEBAR_TOGGLE_RESERVED_PX = 48

  // The rest of the app's top strip (SidebarToggleButton,
  // HeaderBarContainer) is 50px tall. Decorum defaults to 32px with
  // `align-items: end`, which sticks the min/max/close trio to the
  // bottom of its own 32px box — visually misaligned from our 50px
  // header. Stretch the container to 50px and center the buttons so
  // they sit at the same y as the chevron / breadcrumbs / TitleEditor.
  const HEADER_HEIGHT_PX = 50

  const css = `
/* See src/core/decorum-titlebar-fix.ts for context. */
[data-tauri-decorum-tb] {
  pointer-events: none;
  height: ${HEADER_HEIGHT_PX}px !important;
  align-items: center !important;
}
[data-tauri-decorum-tb] > * {
  pointer-events: auto;
}
/* Disable decorum's own drag region. We already have two
   drag-handlers covering the top strip: the SidebarToggleButton's
   TauriDragRegion at z:1 (full top 50px), and HeaderBarContainer's
   manual mousedown handler (z:2, with an interactive-target
   exemption). Decorum's drag region was the third, sitting at z:100
   and ARRIVING LAST — it ate clicks on HeaderBarContainer's buttons
   / inputs / dropdowns because it painted above z:2 and the
   data-tauri-drag-region attribute makes it pre-empt non-drag pointer
   events. Turning pointer-events off here lets clicks fall through
   to the header, while the underlying drag region at z:1 still
   handles dragging the chrome itself. Width/margin overrides kept
   for visual layout in case decorum changes anything we depend on
   later. */
[data-tauri-decorum-tb] > [data-tauri-drag-region] {
  pointer-events: none !important;
  width: calc(100% - ${SIDEBAR_TOGGLE_RESERVED_PX}px) !important;
  margin-left: ${SIDEBAR_TOGGLE_RESERVED_PX}px !important;
}

/* Tighter window controls. Decorum defaults to 58x32; that's wider
   than the Windows 11 native trio (~46x40) and feels chunky next to
   our 28px sidebar chevron. 32px square matches the chevron family.
   border-radius on the button itself (not just on hover) so the
   rounded shape applies the moment hover bg paints — without one,
   the transition flashes a square fill first. Plus 12px right
   margin on the close button so it doesn't sit flush against the
   window edge — visually balances the sidebar's left inset. */
[data-tauri-decorum-tb] .decorum-tb-btn {
  width: 30px !important;
  border-radius: 6px !important;
  /* Default flex-shrink: 1 makes the buttons collapse with the
     drag region when the window narrows. Pin to 0 so they stay
     exactly 30px regardless of width. */
  flex-shrink: 0 !important;
}
[data-tauri-decorum-tb] #decorum-tb-close {
  margin-right: 12px !important;
}

/* Dark-mode icon color: decorum renders the min/max/close glyphs via
   Segoe Fluent Icons in the button's text color (default = black, set
   by the user-agent). In dark mode that's invisible on the dark header
   background. The "dark" class is added to html by ThemeProvider.
   The default-hover background (decorum's rgba(0,0,0,0.2)) also turns
   invisible on dark; swap for a translucent-light hover. The close
   button keeps decorum's own red hover (set with !important in their
   inline CSS), so it wins over the selector below. */
html.dark [data-tauri-decorum-tb] .decorum-tb-btn {
  color: rgba(255, 255, 255, 0.85);
}
html.dark [data-tauri-decorum-tb] .decorum-tb-btn:hover {
  background-color: rgba(255, 255, 255, 0.1);
}
`

  const style = document.createElement('style')
  style.setAttribute('data-ziee-decorum-titlebar-fix', '')
  style.textContent = css
  document.head.appendChild(style)
}
