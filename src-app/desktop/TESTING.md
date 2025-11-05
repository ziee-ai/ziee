# Desktop Testing Guide

**Date**: 2025-11-05
**Status**: Testing Strategy for Tauri Desktop App

---

## Overview

Testing a Tauri desktop app requires different approaches than web apps because of:
- **IPC Communication**: `invoke()` calls between frontend and Rust backend
- **Native Features**: Window management, file dialogs, system tray
- **Platform-Specific**: Different behavior on Linux, Windows, macOS

---

## Testing Layers

```
┌─────────────────────────────────────────────────┐
│  Layer 1: Rust Unit Tests                      │
│  - Test Tauri commands directly                │
│  - No GUI needed                                │
│  - Fast, isolated                               │
└─────────────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────────────┐
│  Layer 2: Frontend Unit Tests (Vitest)         │
│  - Mock Tauri IPC                               │
│  - Test stores and components                  │
│  - Fast, no Tauri needed                        │
└─────────────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────────────┐
│  Layer 3: E2E Tests (WebDriverIO)              │
│  - Real Tauri app                               │
│  - Full IPC communication                       │
│  - Slow, comprehensive                          │
└─────────────────────────────────────────────────┘
```

---

## Layer 1: Rust Unit Tests

Test Tauri commands without launching the app.

### Setup

No additional setup needed - use Rust's built-in test framework.

### Example

```rust
// src-app/desktop/tauri/src/modules/backend/commands.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_server_port() {
        let state = BackendState::new(8080);
        state.set_ready(true);

        let result = get_server_port(tauri::State::from(&state)).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 8080);
    }

    #[tokio::test]
    async fn test_backend_status_not_ready() {
        let state = BackendState::new(8080);
        // Don't set ready

        let result = get_backend_status(tauri::State::from(&state)).await;

        assert!(result.is_ok());
        let status = result.unwrap();
        assert_eq!(status.ready, false);
    }
}
```

### Run

```bash
cd src-app/desktop/tauri
cargo test
```

**Advantages:**
- ✅ Fast (no GUI startup)
- ✅ Test business logic in isolation
- ✅ Easy to debug
- ✅ Works in CI/containers

**Limitations:**
- ❌ Doesn't test IPC layer
- ❌ Doesn't test UI integration

---

## Layer 2: Frontend Unit Tests with Mocked IPC

Test frontend code that uses Tauri, but mock the IPC calls.

### Setup

```bash
cd src-app/desktop/ui
npm install --save-dev vitest @vitest/ui happy-dom
```

**vitest.config.ts:**

```typescript
import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'
import path from 'path'

export default defineConfig({
  plugins: [react()],

  test: {
    globals: true,
    environment: 'happy-dom',
    setupFiles: ['./tests/setup/tauri-mock.ts'],
  },

  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
      '@ziee/ui-core': path.resolve(__dirname, '../../ui/src/index.ts'),
    },
  },
})
```

**Create Tauri mock:**

```typescript
// tests/setup/tauri-mock.ts

import { beforeEach } from 'vitest'

// Mock responses
const mockResponses: Record<string, any> = {
  get_server_port: 8080,
  get_backend_status: { running: true, ready: true, port: 8080 },
  minimize_window: null,
  maximize_window: null,
  is_window_maximized: false,
}

// Setup mock before each test
beforeEach(() => {
  (window as any).__TAURI__ = {
    core: {
      invoke: async (cmd: string, args?: any) => {
        if (cmd in mockResponses) {
          return mockResponses[cmd]
        }
        throw new Error(`Unknown command: ${cmd}`)
      },
    },
    window: {
      getCurrent: () => ({
        isMinimized: async () => false,
        isMaximized: async () => false,
      }),
    },
  }
})

// Helper to override mock responses
export function mockTauriCommand(cmd: string, response: any) {
  mockResponses[cmd] = response
}
```

**Write tests:**

```typescript
// tests/unit/window-store.test.ts

import { describe, it, expect, beforeEach } from 'vitest'
import { useWindowStore } from '@/modules/window/store'
import { mockTauriCommand } from '../setup/tauri-mock'

describe('Window Store', () => {
  beforeEach(() => {
    // Reset store state
    useWindowStore.setState({ isMaximized: false })
  })

  it('should minimize window', async () => {
    const { minimize } = useWindowStore.getState()

    await expect(minimize()).resolves.not.toThrow()
  })

  it('should check maximized state', async () => {
    mockTauriCommand('is_window_maximized', true)

    const { checkIsMaximized } = useWindowStore.getState()
    await checkIsMaximized()

    const { isMaximized } = useWindowStore.getState()
    expect(isMaximized).toBe(true)
  })
})
```

### Run

```bash
npm run test:unit
npm run test:unit:ui  # Interactive UI
```

**Advantages:**
- ✅ Test frontend logic with Tauri integration
- ✅ Fast (no real Tauri app)
- ✅ Easy to mock different scenarios
- ✅ Works in CI/containers

**Limitations:**
- ❌ Doesn't test real IPC communication
- ❌ Mocked responses might not match reality

---

## Layer 3: E2E Tests with Real Tauri IPC

Full integration tests using WebDriverIO.

### Setup

```bash
cd src-app/desktop
mkdir -p tests/e2e
cd tests
npm init -y
npm install --save-dev \
  webdriverio \
  @wdio/cli \
  @wdio/local-runner \
  @wdio/mocha-framework \
  @wdio/spec-reporter \
  chai
```

**wdio.conf.js:**

```javascript
const path = require('path')

exports.config = {
  runner: 'local',

  specs: [
    './e2e/**/*.spec.js'
  ],

  capabilities: [{
    'tauri:options': {
      application: path.resolve(
        __dirname,
        '../tauri/target/debug/ziee-chat'
      )
    }
  }],

  logLevel: 'info',
  framework: 'mocha',
  reporters: ['spec'],

  mochaOpts: {
    timeout: 60000
  },
}
```

**Write E2E tests:**

```javascript
// tests/e2e/backend-ipc.spec.js

const { expect } = require('chai')

describe('Backend IPC Communication', () => {
  it('should get server port via IPC', async () => {
    const port = await browser.execute(async () => {
      return await window.__TAURI__.core.invoke('get_server_port')
    })

    expect(port).to.be.a('number')
    expect(port).to.be.within(8080, 8180)
  })

  it('should verify backend is running', async () => {
    const status = await browser.execute(async () => {
      return await window.__TAURI__.core.invoke('get_backend_status')
    })

    expect(status.running).to.equal(true)
    expect(status.ready).to.equal(true)
  })

  it('should make HTTP request to backend', async () => {
    // Get backend port
    const port = await browser.execute(async () => {
      return await window.__TAURI__.core.invoke('get_server_port')
    })

    // Make HTTP request to backend
    const response = await browser.execute(async (port) => {
      const res = await fetch(`http://127.0.0.1:${port}/api/health`)
      return await res.json()
    }, port)

    expect(response.status).to.equal('healthy')
  })
})
```

### Run

```bash
# Terminal 1: Start tauri-driver
cd src-app/desktop/tauri
cargo tauri driver

# Terminal 2: Run tests
cd src-app/desktop/tests
npm test
```

**Advantages:**
- ✅ Tests real IPC communication
- ✅ Tests full app integration
- ✅ Catches IPC-related bugs

**Limitations:**
- ❌ Slow (app startup overhead)
- ❌ Requires display server (X11/xvfb)
- ❌ More complex setup

---

## Testing in Containers

### For Unit Tests (Rust + Vitest)

No special setup needed:

```bash
# Works out of the box
cargo test
npm run test:unit
```

### For E2E Tests

Use xvfb for headless testing:

```bash
# Install xvfb
sudo apt-get install -y xvfb

# Build app
cd src-app/desktop/tauri
cargo tauri build --debug

# Run driver with xvfb
xvfb-run -a cargo tauri driver &

# Run tests
cd ../tests
xvfb-run -a npm test
```

---

## CI/CD Setup

**GitHub Actions example:**

```yaml
# .github/workflows/desktop-tests.yml
name: Desktop Tests

on: [push, pull_request]

jobs:
  unit-tests-rust:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Run Rust tests
        run: |
          cd src-app/desktop/tauri
          cargo test

  unit-tests-frontend:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions/setup-node@v3
        with:
          node-version: '18'
      - name: Run frontend tests
        run: |
          cd src-app/desktop/ui
          npm install
          npm run test:unit

  e2e-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Install system dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y \
            libwebkit2gtk-4.1-dev \
            xvfb

      - name: Build desktop app
        run: |
          cd src-app/desktop/tauri
          cargo tauri build --debug

      - name: Run E2E tests
        run: |
          cd src-app/desktop/tauri
          xvfb-run -a cargo tauri driver &
          sleep 5
          cd ../tests
          xvfb-run -a npm test
```

---

## Package.json Scripts

**Add to `src-app/desktop/ui/package.json`:**

```json
{
  "scripts": {
    "test:unit": "vitest",
    "test:unit:ui": "vitest --ui",
    "test:unit:coverage": "vitest --coverage"
  }
}
```

**Add to `src-app/desktop/tests/package.json`:**

```json
{
  "scripts": {
    "test": "wdio run wdio.conf.js",
    "test:headed": "wdio run wdio.conf.js --headed"
  }
}
```

---

## Summary

| Layer | Tool | IPC Testing | Speed | Container-Friendly |
|-------|------|-------------|-------|-------------------|
| **Rust Unit** | `cargo test` | ✅ Direct | ⚡ Fast | ✅ Yes |
| **Frontend Unit** | Vitest | ⚠️ Mocked | ⚡ Fast | ✅ Yes |
| **E2E** | WebDriverIO | ✅ Real | 🐌 Slow | ⚠️ Needs xvfb |

**Recommended approach:**
1. **Rust unit tests** for command logic (90% coverage)
2. **Frontend unit tests** for UI components (mocked IPC)
3. **E2E tests** for critical workflows (smoke tests only)

---

**Last Updated**: 2025-11-05
