#!/usr/bin/env node
// check-sse-headers.mjs — regression guard for SSE streaming through an OUTER
// reverse proxy (e.g. a Coder / ingress `nginx` published in front of ziee).
//
// The ziee-web nginx reverse-proxies every `/api` SSE endpoint (chat stream,
// sync subscribe, hardware usage, download/version progress, workflow events).
// Two directives in `location /api` are load-bearing for streaming:
//   - `proxy_buffering off`            → THIS nginx does not buffer the stream.
//   - `add_header X-Accel-Buffering no`→ the disable-buffering signal an
//                                        nginx-family EDGE honors (this nginx
//                                        consumes the axum-set copy, so it must
//                                        be re-emitted here to reach the edge).
// Dropping either silently re-breaks SSE through a buffering edge — the failure
// is invisible on the direct path and only shows through the published URL.
// This script fails the build if either directive leaves `location /api`.
//
// Dependency-free (pure node:fs); run with: node docker/web/check-sse-headers.mjs

import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const confPath = join(here, 'nginx.conf');

/** Strip `#` line comments so a stray brace inside a comment can't desync the
 *  brace matcher. nginx has no block comments; values with literal `#` are not
 *  used in this file. */
function stripComments(conf) {
  return conf.replace(/#[^\n]*/g, '');
}

/** Extract the body of the `location /api { ... }` block (brace-matched). */
function apiLocationBlock(rawConf) {
  const conf = stripComments(rawConf);
  const marker = /location\s+\/api\s*\{/.exec(conf);
  if (!marker) return null;
  let depth = 0;
  let start = -1;
  for (let i = marker.index; i < conf.length; i++) {
    const ch = conf[i];
    if (ch === '{') {
      if (depth === 0) start = i + 1;
      depth++;
    } else if (ch === '}') {
      depth--;
      if (depth === 0) return conf.slice(start, i);
    }
  }
  return null;
}

const REQUIRED = [
  {
    name: 'proxy_buffering off',
    re: /^\s*proxy_buffering\s+off\s*;/m,
    why: 'this nginx must not buffer the SSE stream',
  },
  {
    name: 'add_header X-Accel-Buffering no',
    // The value must be exactly `no` — a nearby token like `no-cache` must NOT
    // satisfy the guard, so require whitespace/`;` after it (not just \b, which
    // treats `-` as a boundary).
    re: /^\s*add_header\s+X-Accel-Buffering\s+no(?=\s|;)/im,
    why: 'the disable-buffering signal an outer/edge nginx honors (Coder published URL)',
  },
];

let conf;
try {
  conf = readFileSync(confPath, 'utf8');
} catch (err) {
  console.error(`check-sse-headers: FAIL — cannot read ${confPath}: ${err.message}`);
  process.exit(1);
}
const block = apiLocationBlock(conf);

const failures = [];
if (block === null) {
  failures.push('could not locate the `location /api { ... }` block in nginx.conf');
} else {
  for (const d of REQUIRED) {
    if (!d.re.test(block)) failures.push(`missing "${d.name}" in location /api — ${d.why}`);
  }
}

if (failures.length) {
  console.error('check-sse-headers: FAIL');
  for (const f of failures) console.error(`  - ${f}`);
  console.error(
    '\nSSE streaming through a buffering edge proxy (e.g. the Coder published URL)\n' +
      'depends on these directives. See the comment in location /api.',
  );
  process.exit(1);
}

console.log('check-sse-headers: OK — location /api keeps proxy_buffering off + X-Accel-Buffering no');
