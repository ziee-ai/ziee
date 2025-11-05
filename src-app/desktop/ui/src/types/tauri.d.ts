/**
 * Tauri Global Type Declarations
 */

declare global {
  interface Window {
    __TAURI__?: {
      invoke: <T = unknown>(
        cmd: string,
        args?: Record<string, unknown>,
      ) => Promise<T>
      event: {
        listen: (
          event: string,
          handler: (event: unknown) => void,
        ) => Promise<() => void>
        emit: (event: string, payload?: unknown) => Promise<void>
      }
    }
  }
}

export {}
