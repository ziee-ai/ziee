/**
 * Vite plugin to detect duplicate form names
 *
 * This plugin scans .tsx files and detects when the same form name is used
 * in multiple places, which can cause ID collisions since Ant Design generates
 * field IDs as {formName}_{fieldName}.
 *
 * Benefits:
 * - Prevents duplicate IDs in the DOM
 * - Ensures unique form identifiers
 * - Catches naming conflicts at build time
 * - Works seamlessly with Vite's watch system
 */

import fs from 'node:fs'
import path from 'node:path'

/**
 * Regex to match <Form name="..." /> or <Form name='...' />
 * Handles various spacing and prop order
 */
const FORM_NAME_PATTERN = /<Form\s+[^>]*name=["']([^"']+)["'][^>]*>/g

/**
 * Extract form names from file content
 */
function extractFormNames(content) {
  const formNames = []
  let match

  FORM_NAME_PATTERN.lastIndex = 0

  while ((match = FORM_NAME_PATTERN.exec(content)) !== null) {
    const formName = match[1]
    if (formName && formName.trim()) {
      formNames.push(formName.trim())
    }
  }

  return formNames
}

/**
 * Recursively find all .tsx files
 */
function findTsxFiles(dir, fileList = []) {
  if (!fs.existsSync(dir)) return fileList

  const files = fs.readdirSync(dir)

  for (const file of files) {
    const filePath = path.join(dir, file)
    const stat = fs.statSync(filePath)

    if (stat.isDirectory()) {
      if (!['node_modules', 'dist', 'build', '.git', 'tests'].includes(file)) {
        findTsxFiles(filePath, fileList)
      }
    } else if (file.endsWith('.tsx')) {
      fileList.push(filePath)
    }
  }

  return fileList
}

/**
 * Scan all files and detect duplicate form names
 */
function checkFormNames(srcDir, logger) {
  try {
    const tsxFiles = findTsxFiles(srcDir)
    const formNameMap = new Map() // Map<formName, string[]> - tracks which files use each form name

    // Collect all form names and their source files
    for (const file of tsxFiles) {
      const content = fs.readFileSync(file, 'utf-8')
      const names = extractFormNames(content)

      for (const name of names) {
        if (!formNameMap.has(name)) {
          formNameMap.set(name, [])
        }
        // Store relative path for better readability
        const relativePath = path.relative(srcDir, file)
        formNameMap.get(name).push(relativePath)
      }
    }

    // Detect duplicates and warn
    const duplicates = []
    for (const [name, files] of formNameMap.entries()) {
      if (files.length > 1) {
        duplicates.push({ name, files })
      }
    }

    if (duplicates.length > 0) {
      logger.warn(`\n[form-names] ⚠️  Found ${duplicates.length} duplicate form name(s):`)
      for (const { name, files } of duplicates) {
        logger.warn(`  • "${name}" defined in:`)
        files.forEach(file => logger.warn(`    - ${file}`))
      }
      logger.warn('\n[form-names] Duplicate names will cause ID collisions in the DOM.')
      logger.warn('[form-names] Ant Design generates IDs as: {formName}_{fieldName}\n')

      // Uncomment to fail build on duplicates:
      // throw new Error(`Duplicate form names found: ${duplicates.map(d => d.name).join(', ')}`)
    } else {
      logger.info(`[form-names] ✓ All form names are unique (${formNameMap.size} forms found)`)
    }

    return { total: formNameMap.size, duplicates: duplicates.length }
  } catch (error) {
    logger.error('[form-names] Failed to check form names:', error)
    return { total: 0, duplicates: 0 }
  }
}

/**
 * Vite plugin
 */
export function formNamesPlugin(options = {}) {
  const { srcDir = 'src' } = options

  let config
  let srcPath
  let debounceTimer

  return {
    name: 'vite-plugin-form-names',

    configResolved(resolvedConfig) {
      config = resolvedConfig
      // config.root is already pointing to 'src', so we use the parent directory
      const rootDir = path.dirname(config.root)
      srcPath = path.resolve(rootDir, srcDir)

      // Check on plugin init
      checkFormNames(srcPath, config.logger)
    },

    handleHotUpdate({ file }) {
      // Re-check when .tsx files change
      if (file.endsWith('.tsx') && !file.includes('node_modules')) {
        // Debounce check (avoid multiple triggers)
        clearTimeout(debounceTimer)
        debounceTimer = setTimeout(() => {
          checkFormNames(srcPath, config.logger)
        }, 1000)
      }
    },

    buildStart() {
      // Check at build start
      checkFormNames(srcPath, config.logger)
    },
  }
}
