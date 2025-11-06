import { test as base } from '@playwright/test'
import { spawn, ChildProcess } from 'child_process'
import { writeFileSync, mkdirSync, existsSync, rmSync, realpathSync } from 'fs'
import { resolve, dirname } from 'path'
import { fileURLToPath } from 'url'
import pg from 'pg'
import crypto from 'crypto'

const { Pool } = pg
const __dirname = dirname(fileURLToPath(import.meta.url))

interface TestInfrastructure {
  databaseName: string
  backendPort: number
  vitePort: number
  baseURL: string
  apiURL: string
  serverProcess: ChildProcess
  viteProcess: ChildProcess
}

interface TestFixtures {
  testInfra: TestInfrastructure
}

// Fixed port assignment per worker
// Each worker always uses the same 2 ports (vite + backend)
// Worker 0: vite 9000, backend 9100
// Worker 1: vite 9001, backend 9101
// Worker 2: vite 9002, backend 9102
// Worker 3: vite 9003, backend 9103
// etc.
function getWorkerPorts(workerIndex: number): { backend: number; vite: number } {
  return {
    vite: 9000 + workerIndex,
    backend: 9100 + workerIndex,
  }
}

export const test = base.extend<TestFixtures>({
  testInfra: async ({}, use, testInfo) => {
    const testId = crypto.randomBytes(4).toString('hex')
    const databaseName = `ziee_test_${testId}`
    const workerIndex = testInfo.workerIndex

    // Each worker always uses the same 2 ports
    const ports = getWorkerPorts(workerIndex)
    const backendPort = ports.backend
    const vitePort = ports.vite

    console.log(`\n🔧 Setting up test infrastructure for: ${testInfo.title}`)
    console.log(`   Database: ${databaseName}`)
    console.log(`   Backend: http://localhost:${backendPort}`)
    console.log(`   Vite: http://localhost:${vitePort}\n`)

    // 1. Create database
    const pool = new Pool({
      host: 'localhost',
      port: 54320,
      user: 'postgres',
      password: 'password',
      database: 'postgres',
    })

    try {
      await pool.query(`CREATE DATABASE ${databaseName}`)
      console.log(`✅ Created database: ${databaseName}`)
    } catch (error) {
      console.error(`❌ Failed to create database ${databaseName}:`, error)
      throw error
    } finally {
      await pool.end()
    }

    // 2. Create backend config file
    const configDir = resolve(__dirname, '../../.test-configs')
    if (!existsSync(configDir)) {
      mkdirSync(configDir, { recursive: true })
    }

    const configPath = resolve(configDir, `test-${testId}.yaml`)
    const configContent = `postgresql:
  use_embedded: false

  external:
    host: "localhost"
    port: 54320
    username: "postgres"
    password: "password"
    database: "${databaseName}"

  pool:
    max_connections: 5
    min_connections: 1
    acquire_timeout_secs: 3
    idle_timeout_secs: 10
    max_lifetime_secs: 60

server:
  host: "127.0.0.1"
  port: ${backendPort}
  api_prefix: "/api"

  cors:
    allow_origins:
      - "http://localhost:${vitePort}"
    allow_methods:
      - "GET"
      - "POST"
      - "PUT"
      - "DELETE"
      - "OPTIONS"
    allow_headers:
      - "Content-Type"
      - "Authorization"

logging:
  level: "info"
  format: "json"

jwt:
  secret: "test-secret-key-for-jwt-tokens-min-32-chars-long-${testId}"
  issuer: "ziee-chat-test"
  audience: "ziee-chat-test-api"
  access_token_expiry_hours: 24
  refresh_token_expiry_days: 30
`

    writeFileSync(configPath, configContent)

    // 3. Start backend server
    console.log(`🚀 Starting backend server on port ${backendPort}...`)

    // Get cargo path (try symlink first, then resolved path)
    const cargoSymlink = process.env.CARGO_HOME
      ? `${process.env.CARGO_HOME}/bin/cargo`
      : `${process.env.HOME}/.cargo/bin/cargo`

    console.log(`Cargo symlink: ${cargoSymlink}`)
    console.log(`Symlink exists: ${existsSync(cargoSymlink)}`)

    // Try using symlink directly instead of realpath
    const cargoPath = cargoSymlink

    console.log(`Using cargo at: ${cargoPath}`)

    const serverProcess = spawn(
      cargoPath,
      ['run', '--bin', 'ziee-chat', '--', '--config-file', configPath],
      {
        cwd: resolve(__dirname, '../../server'),
        stdio: ['ignore', 'pipe', 'pipe'],
        detached: false,
        env: process.env,
      }
    )

    serverProcess.on('error', (error) => {
      console.error(`❌ Backend server error:`, error)
    })

    // Log backend stdout
    serverProcess.stdout?.on('data', (data) => {
      const message = data.toString()
      console.log(`[Backend stdout] ${message}`)
    })

    // Log backend stderr to help debug issues
    serverProcess.stderr?.on('data', (data) => {
      const message = data.toString()
      // Log errors, warnings, and info messages (but not debug)
      if (message.includes('"level":"error"') || message.includes('"level":"warn"') || message.includes('"level":"info"')) {
        console.error(`[Backend stderr] ${message}`)
      }
    })

    // Wait for backend to be ready (120 seconds for cargo compilation on first run)
    const backendReady = await waitForServer(
      `http://localhost:${backendPort}/api/health`,
      120
    )
    if (!backendReady) {
      serverProcess.kill('SIGKILL')
      throw new Error(`Backend server failed to start on port ${backendPort}`)
    }
    console.log(`✅ Backend server ready on port ${backendPort}`)

    // 4. Create Vite config file
    const viteConfigPath = resolve(configDir, `vite-${testId}.ts`)
    const projectRoot = resolve(__dirname, '../..')
    const srcRoot = resolve(projectRoot, 'src')
    const viteConfigContent = `import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import path from 'node:path'

export default defineConfig({
  plugins: [react(), tailwindcss()],
  root: '${srcRoot}',
  resolve: {
    alias: {
      '@': path.resolve('${projectRoot}', './src'),
    },
  },
  server: {
    port: ${vitePort},
    strictPort: true,
    host: '127.0.0.1',
    proxy: {
      '/api/': {
        target: 'http://localhost:${backendPort}',
        changeOrigin: true,
      },
    },
  },
})
`

    writeFileSync(viteConfigPath, viteConfigContent)

    // 5. Start Vite server
    console.log(`🎨 Starting Vite server on port ${vitePort}...`)
    const viteProcess = spawn(
      'npx',
      ['vite', '--config', viteConfigPath, '--clearScreen', 'false'],
      {
        cwd: projectRoot,
        stdio: ['ignore', 'pipe', 'pipe'],
        detached: false,
      }
    )

    viteProcess.on('error', (error) => {
      console.error(`❌ Vite server error:`, error)
    })

    // Wait for Vite to be ready
    const viteReady = await waitForServer(`http://localhost:${vitePort}`, 60)
    if (!viteReady) {
      viteProcess.kill('SIGKILL')
      serverProcess.kill('SIGKILL')
      throw new Error(`Vite server failed to start on port ${vitePort}`)
    }
    console.log(`✅ Vite server ready on port ${vitePort}`)

    const infrastructure: TestInfrastructure = {
      databaseName,
      backendPort,
      vitePort,
      baseURL: `http://localhost:${vitePort}`,
      apiURL: `http://localhost:${backendPort}`,
      serverProcess,
      viteProcess,
    }

    console.log(`✅ Test infrastructure ready!\n`)

    // Run the test
    await use(infrastructure)

    // Cleanup after test
    console.log(`\n🧹 Cleaning up test infrastructure for: ${testInfo.title}`)

    try {
      viteProcess.kill('SIGTERM')
      setTimeout(() => viteProcess.kill('SIGKILL'), 2000)
    } catch {}

    try {
      serverProcess.kill('SIGTERM')
      setTimeout(() => serverProcess.kill('SIGKILL'), 2000)
    } catch {}

    // Drop database
    const cleanupPool = new Pool({
      host: 'localhost',
      port: 54320,
      user: 'postgres',
      password: 'password',
      database: 'postgres',
    })

    try {
      // Terminate all connections to the database
      await cleanupPool.query(`
        SELECT pg_terminate_backend(pg_stat_activity.pid)
        FROM pg_stat_activity
        WHERE pg_stat_activity.datname = '${databaseName}'
          AND pid <> pg_backend_pid()
      `)
      await cleanupPool.query(`DROP DATABASE IF EXISTS ${databaseName}`)
      console.log(`✅ Dropped database: ${databaseName}`)
    } catch (error) {
      console.error(`⚠️  Failed to drop database ${databaseName}:`, error)
    } finally {
      await cleanupPool.end()
    }

    // Clean up config files
    try {
      rmSync(configPath, { force: true })
      rmSync(viteConfigPath, { force: true })
    } catch {}

    // Note: Ports are fixed per worker and will be reused automatically
    // for the next test assigned to this worker

    console.log(`✅ Cleanup complete\n`)
  },
})

export { expect } from '@playwright/test'

async function waitForServer(url: string, maxAttempts: number): Promise<boolean> {
  for (let attempt = 0; attempt < maxAttempts; attempt++) {
    try {
      const response = await fetch(url)
      if (response.status < 500) {
        return true
      }
    } catch (error) {
      // Server not ready yet
    }
    await new Promise(resolve => setTimeout(resolve, 1000))
  }
  return false
}
