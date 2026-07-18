// Shim → @ziee/notification-ui. The full notification inbox page moved to the
// SDK (reusable by any SDK-consuming app; it renders per-kind content via the
// `@ziee/framework/notification` registry, which ziee's `kinds.tsx` populates).
// This `@/`-path shim keeps existing importers (`module.tsx`, the router, the
// gallery coverage registry) unchanged.
export { NotificationsPage } from '@ziee/notification-ui'
