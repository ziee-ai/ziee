// Shim → @ziee/notification-ui. The globally-mounted arrival-toast listener
// moved to the SDK (reusable by any SDK-consuming app). This `@/`-path shim
// keeps existing importers (`module.tsx`, the gallery coverage registry)
// unchanged.
export { NotificationToastListener } from '@ziee/notification-ui'
