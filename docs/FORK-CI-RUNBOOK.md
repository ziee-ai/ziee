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
    # Optional: trigger a `repository_dispatch` to the main ziee repo so an
    # operator/admin is notified to register the new engine version. Engine
    # revisions are NOT tracked in a file — the DB-backed version manager
    # (`llm_local_runtime/runtime_version/`) records each downloaded+verified
    # engine version at runtime. This dispatch is informational only
    # (not yet shipped — Layer 2 follow-up).
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

1. Operator updates the server. The download path's cosign verify runs
   automatically against the new signed release.
2. An admin downloads + registers the new engine version through the
   local-runtime UI (or the `POST /versions/download` API). The
   DB-backed version manager (`llm_local_runtime/runtime_version/`)
   stores the verified version — there is no file to update.
3. The sha256 + cosign signature are verified in-process at download
   time against the release page artifacts.
4. `allow_unsigned_downloads = false` continues to work for the
   now-signed release.

## See also

- The engine binary download/extract/cache + version catalog live under
  `src-app/server/src/modules/llm_local_runtime/engine/` and the
  DB-backed version registry under
  `src-app/server/src/modules/llm_local_runtime/runtime_version/`. The
  former standalone `llm-runtime` crate (and its file-based
  `known_revisions.toml` resolver) was folded into the server module and
  replaced by this runtime version manager, so neither that crate path
  nor a `known_revisions.toml`/`PRE-STAGE-RUNBOOK.md` exists anymore.
- `.github/workflows/code_sandbox.yml` — the exact pattern to copy.
