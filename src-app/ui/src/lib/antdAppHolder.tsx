import { App } from 'antd'
import { useEffect } from 'react'
import type { MessageInstance } from 'antd/es/message/interface'
import type { ModalStaticFunctions } from 'antd/es/modal/confirm'
import type { NotificationInstance } from 'antd/es/notification/interface'

/**
 * Bridge between antd's `App.useApp()` context and non-React code
 * paths (extension hooks, Zustand store actions, async callbacks).
 *
 * The static `message.*` / `Modal.*` / `notification.*` exports from
 * antd v6 work without an `<App>` parent, but they don't pick up the
 * surrounding `ConfigProvider` theme + render outside the React tree.
 * Code paths that need the same toast/modal contract as components
 * use `getAppApi()` here, which returns the live instances captured
 * by `<AntdAppBridge />` (mounted inside `<App>` in ThemeProvider).
 *
 * For code inside a React component, prefer `App.useApp()` directly —
 * this holder exists only to plug the non-React gap.
 */

type AppApi = {
  message: MessageInstance
  modal: Omit<ModalStaticFunctions, 'warn'>
  notification: NotificationInstance
}

let holder: AppApi | null = null

export function getAppApi(): AppApi {
  if (!holder) {
    throw new Error(
      'antd App context not ready — getAppApi() called before <AntdAppBridge /> mounted',
    )
  }
  return holder
}

export function AntdAppBridge() {
  const api = App.useApp()
  useEffect(() => {
    holder = api
    return () => {
      // On unmount (theme switch / hot-reload), the next mount installs
      // the fresh instances. Clearing avoids leaking a stale reference
      // to a torn-down render tree.
      if (holder === api) holder = null
    }
  }, [api])
  return null
}
