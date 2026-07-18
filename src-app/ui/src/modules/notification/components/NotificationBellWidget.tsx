// Shim → @ziee/notification-ui. The generic sidebar notification bell moved to
// the SDK (reusable by any SDK-consuming app). This `@/`-path shim keeps
// existing importers (`module.tsx`, the gallery coverage registry) unchanged.
export { NotificationBellWidget } from '@ziee/notification-ui'
