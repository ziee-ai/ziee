# syntax=docker/dockerfile:1
#
# Ziee sandbox rootfs — Ubuntu 24.04 + Python + R + Node + scientific
# stack. Output is the container's root filesystem, exported as a tar
# and converted to squashfs by tools/build.sh.
#
# Reproducibility: snapshot date and lockfiles are read by build.sh
# from src-app/sandbox-rootfs/pins/ and injected here as build args.
#
# Two flavors:
#   --build-arg ZIEE_SANDBOX_FLAVOR=minimal  (Ubuntu + Python stdlib + util-linux)
#   --build-arg ZIEE_SANDBOX_FLAVOR=full     (full ML/R/Node stack)

ARG UBUNTU_DIGEST=ubuntu:24.04
FROM ${UBUNTU_DIGEST} AS base

ARG ZIEE_SANDBOX_FLAVOR=full
ARG ZIEE_SANDBOX_SCHEMA=1

ENV TZ=UTC
ENV DEBIAN_FRONTEND=noninteractive
ENV LANG=C.UTF-8

# Base utilities present in BOTH flavors. util-linux gives us
# `prlimit` which sandbox.rs uses to apply per-call rlimits.
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates curl wget \
        bash coreutils util-linux \
        bzip2 xz-utils unzip \
        locales tzdata \
    && locale-gen C.UTF-8 \
    && apt-get clean && rm -rf /var/lib/apt/lists/* /var/log/*

# Python stdlib + pip (no ML libs yet — used by both flavors so basic
# tests can `import json; import urllib.request` etc.).
RUN apt-get update && apt-get install -y --no-install-recommends \
        python3 python3-pip python3-venv \
    && apt-get clean && rm -rf /var/lib/apt/lists/* /var/log/*

# ----------------------------------------------------------------------
# Stage: full flavor adds the data-science stack.
# Skipped when ZIEE_SANDBOX_FLAVOR=minimal via the conditional RUN.
# ----------------------------------------------------------------------

RUN if [ "$ZIEE_SANDBOX_FLAVOR" = "full" ]; then \
      set -eux; \
      apt-get update && apt-get install -y --no-install-recommends \
        build-essential gfortran git git-lfs \
        libffi-dev libssl-dev zlib1g-dev \
        vim jq ripgrep fd-find tree \
        net-tools dnsutils iputils-ping \
        gnupg lsb-release apt-transport-https \
      && apt-get clean && rm -rf /var/lib/apt/lists/* /var/log/*; \
    fi

RUN if [ "$ZIEE_SANDBOX_FLAVOR" = "full" ]; then \
      set -eux; \
      pip3 install --no-cache-dir --break-system-packages \
        numpy pandas matplotlib scipy scikit-learn \
        seaborn plotly statsmodels sympy \
        requests httpx beautifulsoup4 \
        ipython jupyter pillow \
        openpyxl xlrd pyarrow; \
    fi

# PyTorch CPU-only — separate layer because it's big (~800 MB) and we
# want cache reuse when other Python pins bump.
RUN if [ "$ZIEE_SANDBOX_FLAVOR" = "full" ]; then \
      set -eux; \
      pip3 install --no-cache-dir --break-system-packages \
        torch torchvision \
        --extra-index-url https://download.pytorch.org/whl/cpu; \
    fi

RUN if [ "$ZIEE_SANDBOX_FLAVOR" = "full" ]; then \
      set -eux; \
      apt-get update && apt-get install -y --no-install-recommends \
        r-base r-base-dev \
      && apt-get clean && rm -rf /var/lib/apt/lists/* /var/log/*; \
    fi

RUN if [ "$ZIEE_SANDBOX_FLAVOR" = "full" ]; then \
      set -eux; \
      Rscript -e "install.packages(c('ggplot2','dplyr','tidyr','readr','stringr','lubridate','purrr','tibble','jsonlite','httr','data.table','caret','forecast'), repos='https://cloud.r-project.org', Ncpus=parallel::detectCores())"; \
    fi

RUN if [ "$ZIEE_SANDBOX_FLAVOR" = "full" ]; then \
      set -eux; \
      curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
      && apt-get install -y --no-install-recommends nodejs \
      && npm install -g typescript ts-node \
      && apt-get clean && rm -rf /var/lib/apt/lists/* /var/log/*; \
    fi

RUN if [ "$ZIEE_SANDBOX_FLAVOR" = "full" ] && command -v fdfind > /dev/null; then \
      ln -s "$(command -v fdfind)" /usr/local/bin/fd; \
    fi

# Schema-version sentinel the server boot probe reads. Bumped only on
# ABI-breaking rootfs changes (Python major, binary paths, layout).
RUN echo "$ZIEE_SANDBOX_SCHEMA" > /.ziee-sandbox-rootfs-schema

# Defense-in-depth: no setuid binaries in the rootfs. bwrap drops caps
# but a setuid binary inside would be surprising. CI Tier 6 asserts
# this with `find / -perm /u+s -o -perm /g+s` returning empty.
RUN find / -xdev \( -perm /u+s -o -perm /g+s \) -type f -exec chmod u-s,g-s {} \; 2>/dev/null || true
