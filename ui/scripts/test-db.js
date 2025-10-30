#!/usr/bin/env node

/**
 * Cross-platform script to manage test PostgreSQL database
 * Works on Windows, macOS, and Linux
 */

import { execSync, spawn } from 'child_process'
import { resolve, join, dirname } from 'path'
import { fileURLToPath } from 'url'

const __dirname = dirname(fileURLToPath(import.meta.url))
const PROJECT_ROOT = resolve(__dirname, '..')
const COMPOSE_FILE = join(PROJECT_ROOT, 'docker-compose.test.yml')

const command = process.argv[2]

function exec(cmd) {
  try {
    execSync(cmd, { stdio: 'inherit', cwd: PROJECT_ROOT })
  } catch (error) {
    process.exit(error.status || 1)
  }
}

function execOutput(cmd) {
  try {
    return execSync(cmd, { cwd: PROJECT_ROOT, encoding: 'utf-8' })
  } catch (error) {
    return ''
  }
}

switch (command) {
  case 'start': {
    console.log('Starting test PostgreSQL...')
    exec(`docker compose -f "${COMPOSE_FILE}" up -d`)
    console.log('Waiting for PostgreSQL to be ready...')

    // Wait for health check
    let ready = false
    for (let i = 0; i < 30; i++) {
      const output = execOutput(
        `docker compose -f "${COMPOSE_FILE}" ps --format json`,
      )
      if (
        output.includes('"Health":"healthy"') ||
        output.includes('(healthy)')
      ) {
        ready = true
        break
      }
      // Sleep 1 second
      execSync('node -e "setTimeout(() => {}, 1000)"')
    }

    if (ready) {
      console.log('✅ Test PostgreSQL is ready on port 54320')
    } else {
      console.log('⚠️  PostgreSQL started but health check timed out')
    }
    break
  }

  case 'stop':
    console.log('Stopping test PostgreSQL...')
    exec(`docker compose -f "${COMPOSE_FILE}" down`)
    console.log('✅ Test PostgreSQL stopped')
    break

  case 'clean':
    console.log('Stopping and removing test PostgreSQL with data...')
    exec(`docker compose -f "${COMPOSE_FILE}" down -v`)
    console.log('✅ Test PostgreSQL cleaned')
    break

  case 'logs': {
    console.log('Showing PostgreSQL logs...')
    const logs = spawn(
      'docker',
      ['compose', '-f', COMPOSE_FILE, 'logs', '-f', 'postgres-test'],
      {
        cwd: PROJECT_ROOT,
        stdio: 'inherit',
      },
    )
    logs.on('exit', code => {
      process.exit(code || 0)
    })
    break
  }

  default:
    console.log('Usage: node scripts/test-db.js {start|stop|clean|logs}')
    console.log('')
    console.log('Commands:')
    console.log('  start  - Start test PostgreSQL container')
    console.log('  stop   - Stop test PostgreSQL container')
    console.log('  clean  - Stop and remove container with all data')
    console.log('  logs   - Show PostgreSQL logs')
    process.exit(1)
}
