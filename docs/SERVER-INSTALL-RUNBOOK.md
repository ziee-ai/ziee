# Ziee Server — Install, Update & Release Runbook

The `ziee` server is Linux-only (static musl, x86_64 + arm64). This covers how
operators install/update it, how the in-app update notification works, and how
maintainers cut a release. (The desktop app is separate — it has its own
auto-updater and is built for all platforms.)

## Install (operators)

One command — detects your CPU arch + distro and installs via the native
package manager or a standalone binary, then sets up systemd:

```bash
curl -fsSL https://github.com/phibya/ziee-chat-new/releases/latest/download/install.sh | sh
```

It picks per distro: `.deb` (Debian/Ubuntu) · `.rpm` (Fedora/RHEL/openSUSE) ·
standalone tarball to `/usr/local/bin` otherwise. Alpine uses the tarball (it
runs OpenRC, not systemd). Flags:
`--version X.Y.Z`, `--method {detect|deb|rpm|standalone}`, `--prefix DIR`,
`--dry-run`.

After a package install:

```bash
sudo systemctl enable --now ziee
journalctl -u ziee -f
# config: /etc/ziee/config.yaml    data: /var/lib/ziee
```

The packaged config binds to **`127.0.0.1:9000`** (localhost only) so a fresh
install isn't exposed before you've set up TLS. To serve externally, put it
behind a reverse proxy (nginx/Caddy) that terminates TLS and set
`server.host: "0.0.0.0"` in `/etc/ziee/config.yaml`. If you use OAuth, also set
`server.trust_forwarded_headers: true` so login redirect URIs are derived from
the proxy's `X-Forwarded-*` headers — leave it `false` when the server is
exposed to clients directly (its security caveat in `config.rs`).

Other options:
- **Docker:** the image ships no config — mount your own at
  `/etc/ziee/config.yaml` and runs as a non-root `ziee` user:
  ```bash
  docker run -v ziee-data:/var/lib/ziee \
    -v $PWD/config.yaml:/etc/ziee/config.yaml:ro \
    -p 9000:9000 ghcr.io/phibya/ziee:latest
  ```
  Defaults to external Postgres + sandbox disabled (see the Dockerfile header).
- **Manual binary:** download `ziee_X.Y.Z_linux_{amd64,arm64}.tar.gz` from the
  release and run `ziee --config-file /path/to/config.yaml`.

### Updating

Re-run the install command (it resolves the latest release). Package installs
can also `apt upgrade` / `dnf upgrade` once a newer package is published.

## Update notification (in-app)

When `update_check.enabled` is true (default), the server checks GitHub's latest
release once a day and caches the result. Admins see:
- a dismissible **banner** at the top of the web UI when a newer version exists, and
- **Settings → About**: current vs latest version, release notes link, and the
  upgrade command above.

It is **notification only** — the server never downloads or installs itself.

**Air-gapped / privacy:** set `update_check: { enabled: false }` in the config to
disable all outbound version checks and the banner.

```yaml
update_check:
  enabled: true   # default; false = no outbound calls, no banner
```

## Release (maintainers)

Releases are unified: **one `vX.Y.Z` tag cuts both the desktop and the server**
into the same GitHub Release (the workspace version is shared).

```bash
git tag v0.2.0
git push origin v0.2.0
```

On the tag:
- `desktop-release.yml` builds + signs the desktop bundles and publishes
  `latest.json` to `gh-pages` (desktop auto-updater).
- `server-release.yml` builds the Linux server (amd64 + arm64 static musl),
  packages it (tarball + `.deb`/`.rpm` via nfpm), pushes a multi-arch
  image to `ghcr.io/phibya/ziee`, and uploads everything + `install.sh` to the
  same `vX.Y.Z` Release.

Both workflows may run on the same tag; the server job creates the Release only
if the desktop job hasn't yet (`gh release create … || true`) and uses
`--clobber` on upload. Desktop assets (`.dmg`/`.msi`/`.AppImage`/`.sig`/
`latest.json`) and server assets never collide by name.

**Asset-name contract** — `install.sh`, `server-release.yml`, and `nfpm` must
agree exactly (a mismatch 404s the install). The server publishes, per arch
(`amd64`/`arm64`):

```
ziee_<version>_linux_<arch>.tar.gz   # standalone
ziee_<version>_linux_<arch>.deb
ziee_<version>_linux_<arch>.rpm
ziee_<version>_checksums.txt         # sha256 of all of the above
```

The workflow forces these exact names via `nfpm --target out/<name>` (nfpm's
default template omits the `linux` infix and uses `x86_64`/`aarch64` for rpm).
`install.sh` downloads `ziee_<version>_checksums.txt` and **sha256-verifies**
each artifact before installing it as root.

**Integrity / signing.** The packages are NOT GPG-signed (no per-distro key
infra) — this matches Coder's installer model. Integrity of the **downloaded
artifacts** (tarballs/packages) rests on: TLS, the sha256 sidecar (`install.sh`
verifies it inline before installing as root), and a Sigstore
**build-provenance attestation** (`actions/attest-build-provenance`) over every
artifact. Note the `install.sh` *script itself* (fetched by `curl … | sh`) is
trusted via GitHub's TLS, like any `curl|sh` installer — it is not separately
signed; pin `--version` and inspect the script first if that matters to you.
Verify an artifact's attestation out-of-band for the strongest check:

```bash
gh attestation verify ziee_<version>_linux_amd64.tar.gz --repo phibya/ziee-chat-new
```

**Seccomp.** Release binaries are built `--no-default-features --features
gpu-detect` (seccomp **off**) — a glibc `libseccomp.a` doesn't link into a
static-musl target. The sandbox still runs with all other hardening (pid-ns,
cgroup, `--clearenv`, rlimits); the startup log shows
`seccomp: off-feature-not-linked`. To get seccomp, build from source on a musl
host with a musl libseccomp.

**First-tag validation (can't be exercised locally):** the cross-arch aarch64
`cargo zigbuild` + embedded-PG download, and the ghcr multi-arch push. Watch the
first real `v*` run.

## Desktop note

The desktop app **embeds the server**, so the server's update notification is
forced OFF there: `config.update_check.enabled = false` in the desktop's
`backend/mod.rs`, and the web `server-update` module is dropped from the desktop
bundle via `CORE_MODULE_BLOCKLIST`. The desktop has its own updater
(`tauri-plugin-updater`) — see `DESKTOP-UPDATER-RUNBOOK.md`.

## Testing (no release needed)

```bash
just check-server-update      # backend unit + install.sh (shellcheck + dry-run + container distro detection)
just check-server-update-int  # mock-GitHub HTTP integration through /api/server-update/status
just check-server-release-ci  # dockerized actionlint over server-release.yml
```
