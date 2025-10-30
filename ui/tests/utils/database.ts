import pg from 'pg'
import { readFileSync } from 'fs'
import { resolve, dirname } from 'path'
import { fileURLToPath } from 'url'

const { Pool } = pg
const __dirname = dirname(fileURLToPath(import.meta.url))
const WORKER_INFO_PATH = resolve(__dirname, '../../.test-workers.json')

export async function getDatabasePool(workerId: number) {
  const allWorkers = JSON.parse(readFileSync(WORKER_INFO_PATH, 'utf-8'))
  const worker = allWorkers[workerId]

  return new Pool({
    host: 'localhost',
    port: 54320,
    user: 'postgres',
    password: 'password',
    database: worker.databaseName,
  })
}

export async function truncateAllTables(workerId: number) {
  const pool = await getDatabasePool(workerId)

  try {
    // Get all table names
    const result = await pool.query(`
      SELECT tablename
      FROM pg_tables
      WHERE schemaname = 'public'
    `)

    const tables = result.rows.map(row => row.tablename)

    // Truncate all tables
    for (const table of tables) {
      await pool.query(`TRUNCATE TABLE ${table} RESTART IDENTITY CASCADE`)
    }
  } finally {
    await pool.end()
  }
}

export async function seedDatabase(workerId: number, seedData: any) {
  const pool = await getDatabasePool(workerId)

  try {
    // Execute seed SQL or insert seed data
    // This is application-specific

    // Example: Create initial admin user
    if (seedData.adminUser) {
      await pool.query(`
        INSERT INTO users (id, username, email, password_hash, is_active, created_at, updated_at)
        VALUES ($1, $2, $3, $4, true, NOW(), NOW())
      `, [
        seedData.adminUser.id,
        seedData.adminUser.username,
        seedData.adminUser.email,
        seedData.adminUser.passwordHash,
      ])
    }
  } finally {
    await pool.end()
  }
}
