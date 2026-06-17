---
name: install-samtools-bcftools
description: Install samtools and bcftools (SAM/BAM/CRAM/VCF/BCF processing) into the code sandbox via bioconda. Use when the user works with sequencing alignment or variant files and these tools aren't installed.
when_to_use: User mentions BAM/SAM/CRAM/VCF/BCF files, read alignments, variant calling, "samtools", "bcftools", "index my BAM", pileup, or genomics file conversion.
metadata: { author: ziee, license: CC0-1.0 }
---

# Install samtools + bcftools

The sandbox rootfs is **read-only** (no `apt`); install into the persistent per-conversation `$HOME` with `micromamba`. Installs survive across `execute_command` calls in the same conversation.

## 1. Ensure micromamba is available

If `command -v micromamba` is empty, bootstrap it (same one-liner as the `setup-datascience-env` skill, step 1).

## 2. Install from bioconda

```bash
micromamba install -y -n base -c conda-forge -c bioconda samtools bcftools
samtools --version | head -1
bcftools --version | head -1
```

Both binaries land in `~/.ziee/micromamba/bin`, already on `PATH` for every later call.

## 3. Typical usage

```bash
samtools view -c input.bam            # count reads
samtools index input.bam              # create .bai
bcftools view -H calls.vcf.gz | head  # peek at variant records
```

## Working with mounted host folders

If the user mounted a folder of big genomics files, it appears under `/mnt/<full host path>` (read-only by default). Read BAM/VCF **in place** — do not copy multi-GB files into the workspace:

```bash
samtools quickcheck /mnt/Users/me/runs/sample.bam && echo OK
```

## Reuse

Check before reinstalling:

```bash
command -v samtools >/dev/null && echo ready || echo "need install"
```

## Notes

- Channel order matters: `-c conda-forge -c bioconda`.
- No `sudo`/`apt`: read-only rootfs by design.
