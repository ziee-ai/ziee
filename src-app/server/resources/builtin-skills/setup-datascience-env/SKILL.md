---
name: setup-datascience-env
description: Bootstrap a persistent Python data-science environment (numpy/pandas/scipy/matplotlib/scikit-learn) inside the code sandbox using micromamba. Use when the user wants Python data analysis, plotting, or scientific computing in chat and the needed packages aren't already installed.
when_to_use: User asks to analyze a CSV/dataframe, plot data, run pandas/numpy/scipy/sklearn, or "set up a Python environment". Run this before reaching for those libraries if an import fails.
metadata: { author: ziee, license: CC0-1.0 }
---

# Set up a persistent Python data-science environment

The sandbox rootfs is **read-only**, so you cannot `apt install` system packages. Instead install into the **persistent per-conversation `$HOME`** with `micromamba` — anything you install survives across `execute_command` calls in the same conversation.

The sandbox already exports `MAMBA_ROOT_PREFIX=~/.ziee/micromamba` and puts `~/.ziee/micromamba/bin` and `~/.local/bin` on `PATH`. So packages installed into the micromamba **base** env are immediately on `PATH` in every later call — no activation needed.

## 1. Bootstrap micromamba (once per conversation)

Only if `command -v micromamba` is empty:

```bash
mkdir -p ~/.local/bin
ARCH=$(uname -m); case "$ARCH" in
  x86_64) MB=linux-64;;
  aarch64|arm64) MB=linux-aarch64;;
  *) echo "unsupported arch $ARCH" >&2; exit 1;;
esac
curl -Ls "https://micro.mamba.pm/api/micromamba/$MB/latest" | tar -xj -C ~/.local bin/micromamba
micromamba --version
```

## 2. Install the data-science stack

```bash
micromamba install -y -n base -c conda-forge \
  python=3.12 numpy pandas scipy matplotlib scikit-learn
```

These land in `~/.ziee/micromamba/bin` (the base env) → already on `PATH`.

## 3. Use it

```bash
python -c "import pandas as pd, numpy as np; print('pandas', pd.__version__)"
```

## Reusing across calls

Do **not** reinstall on every call. At the start of a later call, check first:

```bash
python -c "import pandas" 2>/dev/null && echo ready || echo "need install"
```

Only run step 2 again if it reports `need install` (e.g. a brand-new conversation).

## Notes

- Prefer `-c conda-forge` (add `-c bioconda` for bio tools). Pin `python=` once; don't change it mid-conversation or you'll rebuild the world.
- `pip install <pkg>` also works — it's wired to `--user` (→ `~/.local`, on `PATH`) — for pure-Python packages not on conda-forge.
- No `sudo`, no `apt`: the rootfs is read-only by design.
