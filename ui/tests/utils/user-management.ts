import pg from 'pg'
import { v4 as uuidv4 } from 'uuid'
import bcrypt from 'bcryptjs'

/**
 * Create a user with specific permissions directly in the database
 */
export async function createUserWithPermissions(
  pool: pg.Pool,
  username: string,
  email: string,
  password: string,
  permissions: string[]
): Promise<{ userId: string; groupId: string }> {
  const userId = uuidv4()
  const groupId = uuidv4()
  const passwordHash = await bcrypt.hash(password, 10)

  // Create user
  await pool.query(
    `INSERT INTO users (id, username, email, password_hash, is_active, created_at, updated_at)
     VALUES ($1, $2, $3, $4, true, NOW(), NOW())`,
    [userId, username, email, passwordHash]
  )

  // Create group with permissions
  await pool.query(
    `INSERT INTO groups (id, name, description, permissions, is_system, is_active, created_at, updated_at)
     VALUES ($1, $2, $3, $4, false, true, NOW(), NOW())`,
    [groupId, `${username}_group`, 'Test group', JSON.stringify(permissions)]
  )

  // Assign user to group
  await pool.query(
    `INSERT INTO user_groups (user_id, group_id, assigned_at)
     VALUES ($1, $2, NOW())`,
    [userId, groupId]
  )

  return { userId, groupId }
}

/**
 * Create admin user with all permissions
 */
export async function createAdminUser(
  pool: pg.Pool,
  username: string = 'admin',
  password: string = 'admin123'
): Promise<{ userId: string; groupId: string }> {
  return createUserWithPermissions(
    pool,
    username,
    `${username}@example.com`,
    password,
    ['*']  // Admin has all permissions
  )
}
