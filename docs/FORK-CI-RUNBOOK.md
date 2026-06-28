# Fork CI runbook — engine binary release pipeline

Layer 3 of feat/local-llm-runtime: GitHub Actions workflows in the
**fork repos** at `github.com/ziee-ai/llama.cpp` and
`github.com/ziee-ai/mistral.rs` that build + sign + publish engine
binaries on tag push. This document is the recipe.

The shape is a copy-edit of
`.github/workflows/code_sandbox.yml` in this repo, which already
does the same thing for the code-sandbox rootfs.

## Inputs

- Tag matching `v*` triggers a release.
- Matrix:
  - `platform`: `linux`, `macos`, `windows`
  - `arch`: `x86_64`, `aarch64`
  - `backend`: `cpu`, `cuda`, `rocm`, `metal`, `vulkan` (per platform)

## Artifacts

Each matrix combination produces one archive named exactly:

```
<server>-<platform>-<arch>-<backend>.<ext>
```

- `<server>`: `llama-server` (llamacpp fork) or `mistralrs-server`
  (mistralrs fork).
- `<ext>`: `tar.gz` on Linux/macOS, `zip` on Windows.

These names match the engine download/verify code at
`src-app/server/src/modules/llm_local_runtime/engine/download.rs` (the
standalone `llm-runtime` crate was folded into the server module).

## Signing

Cosign keyless signing using the Actions OIDC issuer. Each archive
gets a sibling `.sig` file uploaded to the same release. The
verifier in `llm_local_runtime/engine/download.rs` (P1.l of this PR)
uses the `sigstore` Rust crate to validate against the OIDC issuer
`https://token.actions.githubusercontent.com` and the cert-identity
regex `^https://github.com/<repo>/.github/workflows/.*@refs/tags/<tag>$`.

## Workflow skeleton

```yaml
name: release engine binary

on:
  push:
    tags: ['v*']
  workflow_dispatch:
    inputs:
      tag:
        description: 'Tag to publish'
        required: true

permissions:
  contents: write   # gh release upload
  id-token: write   # cosign keyless OIDC

jobs:
  release:
    name: build + sign + publish (${{ matrix.platform }}-${{ matrix.arch }}-${{ matrix.backend }})
    strategy:
      fail-fast: false
      matrix:
        include:
          - platform: linux
            arch: x86_64
            backend: cpu
            runs-on: ubuntu-22.04
          - platform: linux
            arch: x86_64
            backend: cuda
            runs-on: ubuntu-22.04
          # ... etc — see code_sandbox.yml for the full matrix shape
    runs-on: ${{ matrix.runs-on }}
    steps:
      - uses: actions/checkout@v4
      - name: build engine
        run: |
          # llamacpp-specific build steps; substitute for mistralrs.
          cmake -B build -DGGML_${{ matrix.backend == 'cuda' && 'CUDA=ON' || 'CPU=ON' }}
          cmake --build build --target llama-server -j
      - name: package
        run: |
          ARCHIVE=llama-server-${{ matrix.platform }}-${{ matrix.arch }}-${{ matrix.backend }}.tar.gz
          tar -czvf $ARCHIVE -C build/bin llama-server
          echo "ARCHIVE=$ARCHIVE" >> $GITHUB_ENV
      - name: cosign sign-blob
        uses: sigstore/cosign-installer@v3
      - run: |
          cosign sign-blob --yes --output-signature ${ARCHIVE}.sig $ARCHIVE
      - name: upload to release
        uses: softprops/action-gh-release@v2
        with:
          files: |
            ${{ env.ARCHIVE }}
            ${{ env.ARCHIVE }}.sig

  receiver:
    # Optional: trigger a `repository_dispatch` to the main ziee-chat
    # repo so its receiver workflow (Layer 2) opens an auto-PR
    # updating known_revisions.toml. See ziee-chat/.github/workflows
    # for the receiver shape (not yet shipped — Layer 2 follow-up).
    needs: release
    runs-on: ubuntu-22.04
    steps:
      - name: dispatch
        uses: peter-evans/repository-dispatch@v3
        with:
          token: ${{ secrets.RECEIVER_PAT }}
          repository: phibya/ziee-chat-new
          event-type: llm-runtime-release
          client-payload: |
            { "engine": "llamacpp", "tag": "${{ github.ref_name }}" }
```

## Verification

Once a release ships:

1. Operator updates the server. Layer 1 cosign verify runs
   automatically.
2. The new revision is registered in the DB-backed version catalog
   (`llm_local_runtime/runtime_version/` + `version_manager.rs`), which
   records the sha256s the download/verify path checks. The file-based
   `known_revisions.toml` resolver no longer exists (see the note in
   **See also** below) — there is no TOML to hand-edit.
3. The recorded sha256s are confirmed against the release page before
   the revision is marked available.
4. With `allow_unsigned_downloads = false`, only the now-signed,
   sha256-verified release is downloadable.

## See also

- The engine binary download/extract/cache + version catalog now live
  under `src-app/server/src/modules/llm_local_runtime/engine/` (the
  standalone `llm-runtime` crate, along with its `PRE-STAGE-RUNBOOK.md`
  and `known_revisions.toml`, was folded into the server module).
- `.github/workflows/code_sandbox.yml` — the exact pattern to copy.

> Note: earlier revisions of this runbook pointed at
> `src-app/llm-runtime/PRE-STAGE-RUNBOOK.md` and
> `src-app/llm-runtime/known_revisions.toml`. The standalone `llm-runtime`
> crate was folded into `server` and the file-based `known_revisions.toml`
> resolver was replaced by the DB-backed version manager
> (`code_sandbox/version_manager.rs`), so those paths no longer exist.
