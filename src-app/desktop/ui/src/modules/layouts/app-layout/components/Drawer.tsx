/**
 * DELIBERATE DIVERGENCE from core's Drawer.
 *
 * Desktop is a superset of core: the Radix Dialog structure, size /
 * width resolution, footer normalization, mask + body styling all
 * match core 1:1 (core was ported off antd to Radix; this mirrors
 * it). The desktop-only additions are:
 *
 *   - Manual `startDragging()` mousedown on the drawer title strip
 *     (with interactive-target exemption for the close Button and
 *     any future controls). Matches HeaderBarContainer.
 *   - `titleRef` + ResizeObserver effect that watches the drawer's
 *     left edge and adds left padding when the drawer would sit
 *     under the macOS traffic-light controls (clears 72px on Mac).
 *   - `resizeMaxWidth` passed to ResizeHandle so dragging the left
 *     edge can't push the drawer under the traffic lights either.
 *   - `wrapper` maxWidth / border / margin formulas that account for
 *     Tauri window chrome (90px reserve on Mac).
 *
 * If you find behavior that core has and desktop doesn't (a real
 * regression rather than a deliberate addition), copy core's logic
 * into the matching place here. `just desktop-drift-check` will flag
 * the file as long as it differs at all — the marker above tells the
 * recipe the difference is intentional.
 */

import * as React from 'react'
import { useCallback, useEffect, useRef } from 'react'
import * as DialogPrimitive from '@radix-ui/react-dialog'
import { Button, Title } from '@/components/ui'
import { ResizeHandle } from '@/modules/layouts/app-layout/components/ResizeHandle'
import { useWindowMinSize } from '@/modules/layouts/app-layout/hooks/useWindowMinSize'
import { IoIosArrowBack } from 'react-icons/io'
import { DivScrollY } from '@/components/common/DivScrollY'
import { isTauriView, isMacOS, isLinux } from '@ziee/desktop/core/platform'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { cn } from '@/lib/utils'

const INTERACTIVE_SEL =
  'button, a, input, textarea, select, [role="button"], [role="link"], [role="menuitem"], [role="combobox"], [contenteditable="true"]'

/**
 * Pure decision for the stacking guard: is any layer OTHER than `self` stacked
 * strictly above `thisZ`? Extracted so the guard's logic is unit-testable
 * without a real DOM (the DOM query/parse is done by the caller). See DRIFT-1.7.
 */
export function isHigherLayerPresent(
  layers: { el: Element; z: number }[],
  self: Element | null,
  thisZ: number,
): boolean {
  for (const { el, z } of layers) {
    if (el === self) continue
    if (Number.isFinite(z) && z > thisZ) return true
  }
  return false
}

type Placement = 'left' | 'right' | 'top' | 'bottom'

// Local app Drawer. Public API preserved from the previous antd-backed
// version (open/onClose/title/placement/size/footer/mask/extra/styles/
// classNames/...); internals now run on Radix Dialog (the same primitive
// the kit Sheet uses), so the ~28 consumers stay unchanged.
export interface DrawerProps {
  open?: boolean
  onClose?: () => void
  title?: React.ReactNode
  /**
   * Accessible name when `title` is a non-string node (Radix requires a
   * `Dialog.Title`; a node can't be introspected for its text).
   */
  titleText?: string
  placement?: Placement
  /** Panel size on the resize axis: a px number, or legacy 'default'(378)/'large'(736). */
  size?: number | 'default' | 'large'
  /** Explicit width (overrides `size`); legacy escape hatch. */
  width?: number | string
  footer?: React.ReactNode
  extra?: React.ReactNode
  /** Backdrop. `false` = no overlay (non-modal); `{ closable:false }` = don't close on backdrop click. */
  mask?: boolean | { closable?: boolean }
  /** Close on backdrop click (legacy `maskClosable`). Overrides `mask.closable`. */
  maskClosable?: boolean
  /** Show the header close affordance (legacy `closable`). Default true. */
  closable?: boolean
  className?: string
  classNames?: { body?: string; header?: string; footer?: string; wrapper?: string }
  styles?: {
    body?: React.CSSProperties
    header?: React.CSSProperties
    footer?: React.CSSProperties
    wrapper?: React.CSSProperties
  }
  /** Unmount children when closed (legacy `destroyOnHidden`). Default true (Radix unmounts on close). */
  destroyOnHidden?: boolean
  zIndex?: number
  /** Render children directly (caller owns scrolling) instead of inside the DivScrollY layer. */
  noBodyScrollWrap?: boolean
  /** Overrides the default `layout-drawer-content` testid on the content root. */
  'data-testid'?: string
  children?: React.ReactNode
}

const sizePx = (size: DrawerProps['size']): number =>
  size === 'default' ? 378 : size === 'large' ? 736 : typeof size === 'number' ? size : 520

const sidePos: Record<Placement, string> = {
  right: 'inset-y-0 right-0 h-full data-[state=open]:slide-in-from-right data-[state=closed]:slide-out-to-right',
  left: 'inset-y-0 left-0 h-full data-[state=open]:slide-in-from-left data-[state=closed]:slide-out-to-left',
  top: 'inset-x-0 top-0 w-full data-[state=open]:slide-in-from-top data-[state=closed]:slide-out-to-top',
  bottom: 'inset-x-0 bottom-0 w-full data-[state=open]:slide-in-from-bottom data-[state=closed]:slide-out-to-bottom',
}

export const Drawer: React.FC<DrawerProps> = ({
  open,
  onClose,
  title,
  titleText,
  placement = 'right',
  size,
  width,
  footer,
  extra,
  mask = true,
  maskClosable: maskClosableProp,
  closable = true,
  className,
  classNames,
  styles,
  zIndex,
  noBodyScrollWrap = false,
  'data-testid': testid,
  children,
}) => {
  const windowMinSize = useWindowMinSize()
  const horizontal = placement === 'left' || placement === 'right'

  const drawerDivRef = useRef<HTMLDivElement>(null)
  const titleRef = useRef<HTMLDivElement>(null)

  // Monitor the drawer's left edge and add title padding so the
  // header clears the macOS traffic-light controls.
  useEffect(() => {
    if (!isTauriView) return
    if (!open) return

    const monitorPosition = () => {
      if (drawerDivRef.current && titleRef.current) {
        const rect = drawerDivRef.current.getBoundingClientRect()
        const leftMin = isMacOS ? 72 : 0
        if (rect.left < leftMin) {
          titleRef.current.style.paddingLeft = leftMin - rect.left + 'px'
        } else {
          titleRef.current.style.paddingLeft = ''
        }
      }
    }

    // Run after the drawer animation completes to get the final position.
    const initialTimeout = setTimeout(monitorPosition, 300)
    const resizeObserver = new ResizeObserver(monitorPosition)
    if (drawerDivRef.current) resizeObserver.observe(drawerDivRef.current)

    return () => {
      clearTimeout(initialTimeout)
      resizeObserver.disconnect()
    }
  }, [open])

  // Leave room for the traffic lights on macOS when resizing.
  const resizeMaxWidth =
    isTauriView && isMacOS ? window.innerWidth - 90 : window.innerWidth - 24

  // Window-drag handlers for the title strip — same pattern as
  // HeaderBarContainer. Bail on interactive descendants so the close
  // Button (and any future header controls) keep working.
  const handleTitleMouseDown = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      if (!isTauriView) return
      if (isLinux) return
      if (e.button !== 0) return
      const target = e.target as Element
      if (target.closest?.(INTERACTIVE_SEL)) return
      e.preventDefault()
      void getCurrentWindow().startDragging()
    },
    [],
  )

  const handleTitleDoubleClick = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      if (!isTauriView) return
      if (isLinux) return
      const target = e.target as Element
      if (target.closest?.(INTERACTIVE_SEL)) return
      void getCurrentWindow().toggleMaximize()
    },
    [],
  )

  const maskClosable =
    maskClosableProp ?? (typeof mask === 'object' ? mask.closable !== false : mask !== false)
  const showOverlay = mask !== false

  // A drawer must NOT dismiss (Escape / click-outside) while another drawer or
  // dialog is stacked ABOVE it — e.g. a file preview opened from inside this
  // drawer. Radix fires this lower layer's dismiss handlers too; guard them so
  // closing the top layer doesn't also close this one. (Ported back from core —
  // the desktop shadow had dropped it; see DRIFT-1.7.)
  const thisZ = zIndex ?? 50
  const higherLayerOpen = () => {
    // Key drawer detection off the STABLE `data-slot="layout-drawer"` marker, not
    // the caller-overridable `data-testid` — an overridden testid must not make a
    // stacked drawer invisible to the guard (audit finding). Other overlay kinds
    // are matched by their own stable data-slots.
    const layers = [
      ...document.querySelectorAll(
        '[data-slot="layout-drawer"], [data-slot="dialog-content"], [data-slot="alert-dialog-content"], [data-slot="sheet-content"]',
      ),
    ].map(el => ({ el, z: parseInt(getComputedStyle(el).zIndex, 10) }))
    return isHigherLayerPresent(layers, drawerDivRef.current, thisZ)
  }

  // px size on the resize axis; full-bleed on the smallest breakpoint.
  const axisPx = width ?? (windowMinSize.xs && horizontal ? '100%' : sizePx(size))
  const sizeStyle: React.CSSProperties = horizontal ? { width: axisPx } : { height: axisPx }

  // Floating-card insets matching the LeftSidebar, with extra reserve
  // for Tauri window chrome (90px on Mac for the traffic lights).
  const reserve = isTauriView ? 90 : 24
  const wrapperStyle: React.CSSProperties = windowMinSize.xs
    ? {
        border: isTauriView ? '1px solid var(--border)' : 'none',
        borderRadius: isTauriView ? 8 : 0,
        maxWidth: '100vw',
        margin: 0,
      }
    : {
        border: '1px solid var(--border)',
        borderRadius: 8,
        maxWidth: `calc(100vw - ${reserve}px)`,
        marginTop: 8,
        marginRight: 8,
        marginBottom: 8,
        marginLeft: 12,
      }

  const footerNode = Array.isArray(footer) ? (
    <div className="flex gap-2">
      {footer.map((item, i) => (
        <React.Fragment key={i}>{item}</React.Fragment>
      ))}
    </div>
  ) : (
    footer
  )

  const body = (
    <div className={cn('flex w-full h-full pr-3', classNames?.body)} style={styles?.body}>
      {React.Children.map(children, child =>
        React.isValidElement<{ className?: string }>(child)
          ? React.cloneElement(child, {
              ...child.props,
              className: `w-full ${child.props.className || ''}`.trim(),
            })
          : child,
      )}
    </div>
  )

  return (
    <DialogPrimitive.Root open={open} onOpenChange={o => { if (!o) onClose?.() }}>
      <DialogPrimitive.Portal>
        {showOverlay && (
          <DialogPrimitive.Overlay
            className="fixed inset-0 z-50 bg-background/75 backdrop-brightness-75 data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0"
          />
        )}
        <DialogPrimitive.Content
          ref={drawerDivRef}
          // Stable marker for "an app Drawer is open" + the target the stacking
          // guard's querySelector looks for. NOT the data-testid (a caller can
          // override that, which would silently break the guard).
          data-slot="layout-drawer"
          data-testid={testid ?? 'layout-drawer-content'}
          // maskClosable=false → backdrop/outside click doesn't dismiss (Escape still does).
          // A higher-stacked drawer/dialog (e.g. a file preview opened from here)
          // being closed must not also dismiss THIS drawer.
          onEscapeKeyDown={e => { if (higherLayerOpen()) e.preventDefault() }}
          onPointerDownOutside={e => { if (!maskClosable || higherLayerOpen()) e.preventDefault() }}
          onInteractOutside={e => { if (!maskClosable || higherLayerOpen()) e.preventDefault() }}
          style={{ ...sizeStyle, ...wrapperStyle, zIndex, ...styles?.wrapper }}
          className={cn(
            'fixed z-50 flex flex-col gap-0 bg-background shadow-none transition ease-in-out overflow-hidden',
            'data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=open]:duration-500 data-[state=closed]:duration-300',
            sidePos[placement],
            className,
            classNames?.wrapper,
          )}
        >
          {title != null && (
            <div
              ref={titleRef}
              className={cn('flex w-full items-center gap-1 relative px-1 py-2 pt-[10px]', classNames?.header)}
              style={{
                paddingLeft:
                  windowMinSize.xs && isTauriView && isMacOS ? 74 : undefined,
                ...styles?.header,
              }}
              onMouseDown={handleTitleMouseDown}
              onDoubleClick={handleTitleDoubleClick}
            >
              {typeof title === 'string' ? (
                // The visible heading IS the dialog's accessible name.
                <DialogPrimitive.Title asChild>
                  <Title level={5} className="!m-0">{title}</Title>
                </DialogPrimitive.Title>
              ) : (
                // A node title can't be introspected for text — render it
                // visually and label the dialog via an sr-only Title so Radix
                // still gets an accessible name (aria-labelledby).
                <>
                  <DialogPrimitive.Title className="sr-only">{titleText ?? 'Drawer'}</DialogPrimitive.Title>
                  {title}
                </>
              )}
              {/* Header-action + close cluster, right-aligned. J7: the close
                  affordance is standardized to the RIGHT to match the dialog /
                  sheet / panel majority rather than the old left-of-title spot. */}
              <div className="ms-auto flex items-center gap-1">
                {extra != null && <div>{extra}</div>}
                {closable && (
                  <Button variant="ghost" size="icon" tooltip="Close" aria-label="Close drawer" onClick={onClose} className="w-[30px]" data-testid="desktop-layout-drawer-close">
                    <span className="text-xl"><IoIosArrowBack aria-hidden="true" /></span>
                  </Button>
                )}
              </div>
            </div>
          )}

          <div className="flex-1 min-h-0 pl-3 pr-0 pt-0 overflow-x-visible">
            {noBodyScrollWrap ? body : <DivScrollY className="flex w-full h-full">{body}</DivScrollY>}
          </div>

          {footerNode != null && (
            <div className={cn('px-3 pb-3 pt-1.5', classNames?.footer)} style={styles?.footer}>
              {footerNode}
            </div>
          )}

          {/* hidden a11y title when caller passes none (Radix requires a labelled dialog) */}
          {title == null && <DialogPrimitive.Title className="sr-only">Drawer</DialogPrimitive.Title>}

          <ResizeHandle placement="left" parentLevel={[1]} maxWidth={resizeMaxWidth} />
        </DialogPrimitive.Content>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  )
}
