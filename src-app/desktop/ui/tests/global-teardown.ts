/**
 * Playwright global-teardown — stop the shared Postgres container,
 * release locks, and wipe leftover config files. Symmetrical to
 * `global-setup.ts`.
 *
 * Per-test backend processes are stopped by their own fixture cleanup
 * in test-context.ts; this teardown only handles the global stuff.
 */

import { execSync } from 'child_process'
import { existsSync, readFileSync, unlinkSync } from 'fs'
import { resolve, dirname } from 'path'
import { fileURLToPath } from 'url'
import { releasePostgresPortLock } from './fixtures/port-manager'

const __dirname = dirname(fileURLToPath(import.meta.url))

export default async function globalTeardown() {
  console.log('\n🧹 Tearing down desktop test infrastructure...\n')

  const runId = process.env.TEST_RUN_ID
  if (!runId) {
    console.log('⚠️  No TEST_RUN_ID found — skipping teardown')
    return
  }

  const configDir = resolve(__dirname, '.test-configs')
  const postgresConfigPath = resolve(configDir, `postgres-${runId}.json`)

  if (existsSync(postgresConfigPath)) {
    try {
      const cfg = JSON.parse(readFileSync(postgresConfigPath, 'utf-8'))

      // Stop + remove the Docker container.
      if (existsSync(cfg.dockerComposePath)) {
        console.log(
          `🐘 Stopping PostgreSQL container for run ${runId}...`,
        )
        try {
          execSync(
            `docker compose -f "${cfg.dockerComposePath}" down -v`,
            { stdio: 'inherit' },
          )
        } catch {
          console.warn('⚠️  docker compose down failed (continuing)')
        }
      }

      releasePostgresPortLock(cfg.port)

      // Wipe per-run config files.
      try {
        unlinkSync(postgresConfigPath)
      } catch {}
      if (existsSync(cfg.dockerComposePath)) {
        try {
          unlinkSync(cfg.dockerComposePath)
        } catch {}
      }
    } catch (err) {
      console.warn('⚠️  Failed to read Postgres config during teardown:', err)
    }
  }

  console.log('✅ Desktop test cleanup complete!\n')
}
