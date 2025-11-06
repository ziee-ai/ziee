import { FullConfig } from '@playwright/test'
import { execSync } from 'child_process'
import { resolve, dirname } from 'path'
import { fileURLToPath } from 'url'
import pg from 'pg'
import dotenv from 'dotenv'
import { cleanupStaleLocks } from './fixtures/port-manager'

const { Pool } = pg
const __dirname = dirname(fileURLToPath(import.meta.url))

export default async function globalSetup(_config: FullConfig) {
  // Load environment variables from .env.test
  dotenv.config({ path: resolve(__dirname, '.env.test') })

  console.log('\n🚀 Starting Playwright E2E Test Infrastructure...\n')

  // Clean up stale port locks from previous crashed/killed test runs
  cleanupStaleLocks()

  // 1. Start Docker PostgreSQL (single instance for all tests)
  console.log('📦 Starting Docker PostgreSQL...')
  try {
    execSync('node scripts/test-db.js start', {
      cwd: resolve(__dirname, '..'),
      stdio: 'inherit',
    })
  } catch (error) {
    console.error('❌ Failed to start PostgreSQL')
    throw error
  }

  // Wait for PostgreSQL to be fully ready
  await new Promise(resolve => setTimeout(resolve, 2000))

  // 2. Verify PostgreSQL connection
  const pool = new Pool({
    host: 'localhost',
    port: 54321,
    user: 'postgres',
    password: 'password',
    database: 'postgres',
  })

  try {
    await pool.query('SELECT 1')
    console.log('✅ Connected to test PostgreSQL\n')
  } catch (error) {
    console.error('❌ Failed to connect to PostgreSQL:', error)
    throw error
  } finally {
    await pool.end()
  }

  console.log('✅ PostgreSQL ready for tests!\n')
  console.log('   Test infrastructure:')
  console.log('   - Each worker: 2 fixed ports (vite + backend)')
  console.log('   - Each test: unique database + backend restart')
  console.log('   - Worker 0: 9000+9100, Worker 1: 9001+9101, etc.\n')
}
