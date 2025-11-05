import { test as base } from '@playwright/test'
import { readFileSync } from 'fs'
import { resolve, dirname } from 'path'
import { fileURLToPath } from 'url'

const __dirname = dirname(fileURLToPath(import.meta.url))
const WORKER_INFO_PATH = resolve(__dirname, '../../.test-workers.json')

interface WorkerInfo {
  workerId: number
  port: number
  vitePort: number
  databaseName: string
  serverPid: number
  vitePid: number
}

interface WorkerFixtures {
  workerInfo: WorkerInfo
  baseURL: string
  apiURL: (path: string) => string
}

// Extend base test with worker-specific fixtures
export const test = base.extend<WorkerFixtures>({
  workerInfo: async ({}, use, workerInfo) => {
    const allWorkers: WorkerInfo[] = JSON.parse(
      readFileSync(WORKER_INFO_PATH, 'utf-8')
    )

    // Use modulo to map any worker index to one of our configured workers
    // This allows Playwright to create more worker processes than we have backend servers
    // For example: worker index 5 maps to backend server 1 (5 % 4 = 1)
    const workerIndex = workerInfo.workerIndex % allWorkers.length
    const worker = allWorkers[workerIndex]

    if (!worker) {
      throw new Error(`No worker info found for computed index ${workerIndex} (original: ${workerInfo.workerIndex})`)
    }

    await use(worker)
  },

  baseURL: async ({ workerInfo }, use) => {
    // Use Vite port for frontend access, which proxies API requests to backend
    const baseURL = `http://localhost:${workerInfo.vitePort}`
    await use(baseURL)
  },

  apiURL: async ({ baseURL }, use) => {
    const apiURL = (path: string) => {
      const cleanPath = path.startsWith('/') ? path : `/${path}`
      return `${baseURL}/api${cleanPath}`
    }
    await use(apiURL)
  },
})

export { expect } from '@playwright/test'
