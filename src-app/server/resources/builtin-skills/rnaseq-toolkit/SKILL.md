---
name: rnaseq-toolkit
description: Install a bulk RNA-seq toolkit (STAR, salmon, HISAT2, featureCounts/subread, samtools) into the code sandbox via bioconda. Use when the user runs RNA-seq alignment or quantification and the tools aren't installed.
when_to_use: User mentions RNA-seq, transcript quantification, read alignment for expression, STAR, salmon, HISAT2, featureCounts, subread, or "FASTQ to counts".
metadata: { author: ziee, license: CC0-1.0 }
---

# RNA-seq toolkit

Read-only rootfs (no `apt`) → install into the persistent per-conversation `$HOME` with `micromamba`. Survives across `execute_command` calls in the conversation.

## 1. Ensure micromamba

Bootstrap if `command -v micromamba` is empty (see `setup-datascience-env`, step 1).

## 2. Install the toolkit

```bash
micromamba install -y -n base -c conda-forge -c bioconda \
  star salmon hisat2 subread samtools
```

(`subread` provides `featureCounts`.) All land on `PATH` (`~/.ziee/micromamba/bin`).

## 3. Sanity check

```bash
STAR --version; salmon --version; featureCounts -v 2>&1 | head -1
```

## Typical flow (salmon, lightweight)

```bash
salmon index -t transcripts.fa -i salmon_index
salmon quant -i salmon_index -l A -r reads.fastq.gz -o quant_out
```

Large references / FASTQ are usually a **mounted host folder** under `/mnt/<full host path>` (read-only) — read inputs there and write outputs into the workspace (`~`), which is writable. STAR genome indexing is memory-hungry; mind the sandbox memory cap (Admin → Code Sandbox → Resource limits).

## Reuse

```bash
command -v salmon >/dev/null && echo ready || echo "need install"
```

## Notes

- Channel order: `-c conda-forge -c bioconda`.
- No `sudo`/`apt`: read-only rootfs by design.
