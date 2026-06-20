-- Citation Management + Verification — the user-level bibliography library.
--
-- The full bibliographic record is stored as a single CSL-JSON item in the
-- `csl_json` JSONB column (the source of truth; the schema Zotero/Mendeley/
-- pandoc all speak). The scalar columns are a PROJECTION of csl_json, written
-- at the same time, so the database can do what JSONB can't cheaply: dedup
-- via partial-unique indexes, per-user citation_key uniqueness, fast
-- sorting/filtering, and full-text search over the title.
--
-- Dedup keys (see modules/citations/dedup): normalized DOI > PMID exact > a
-- fingerprint for identifier-less entries. (PMCID/arXiv are stored for display
-- but resolve to a DOI during resolution, so dedup collapses on the DOI.) The
-- partial-unique indexes make the DOI/PMID/exact-fingerprint cases atomic +
-- race-safe; fuzzy near-matches are surfaced for user review (no DB constraint).

CREATE TABLE bibliography_entries (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id             UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    csl_json            JSONB NOT NULL,                  -- canonical record (citeproc-native)

    -- Projected from csl_json at write time (for queries / dedup / constraints):
    doi                 TEXT,                            -- normalized: lowercased, scheme-stripped
    pmid                TEXT,
    pmcid               TEXT,
    arxiv_id            TEXT,
    title               TEXT,
    year                INTEGER,
    dedup_fingerprint   TEXT,                            -- hash(normTitle)|author1|year; NULL when doi/pmid present
    citation_key        TEXT NOT NULL,                   -- surnameYEAR (collision-suffixed)

    verification_status TEXT NOT NULL DEFAULT 'unverified'
        CHECK (verification_status IN ('unverified','verified','mismatch','not_found')),
    verified_at         TIMESTAMPTZ,
    source              TEXT,                            -- import | doi | pmid | manual | lit_search

    content_tsv         tsvector GENERATED ALWAYS AS (to_tsvector('english', coalesce(title, ''))) STORED,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Dedup race-guards: a user can't hold two rows for the same identifier.
CREATE UNIQUE INDEX uq_bibliography_user_doi
    ON bibliography_entries (user_id, lower(doi)) WHERE doi IS NOT NULL;
CREATE UNIQUE INDEX uq_bibliography_user_pmid
    ON bibliography_entries (user_id, pmid) WHERE pmid IS NOT NULL;
-- Atomic exact-duplicate guard for identifier-less entries.
CREATE UNIQUE INDEX uq_bibliography_user_fingerprint
    ON bibliography_entries (user_id, dedup_fingerprint)
    WHERE doi IS NULL AND pmid IS NULL AND dedup_fingerprint IS NOT NULL;
-- Stable per-user citation key (the [@key] the model/author references).
CREATE UNIQUE INDEX uq_bibliography_user_citation_key
    ON bibliography_entries (user_id, citation_key);

-- Listing + search.
CREATE INDEX idx_bibliography_user ON bibliography_entries (user_id);
CREATE INDEX idx_bibliography_tsv ON bibliography_entries USING GIN (content_tsv);
