-- EA-schema structural fingerprint (catalog-derived, name/order-invariant).
-- Emits a deterministic, sorted, diffable text image of the PUBLIC-schema
-- structure per EXTRACTION_CHECK_SPEC §3 EA-schema. Run with:
--   psql "$URL" -X -q -t -A -F $'\t' -f schema_fp.sql
-- Every section is globally ORDER BY'd so identical schemas produce
-- byte-identical output regardless of attnum order, emission order, or
-- auto-generated constraint/index names.
--
-- RLS / GRANT / COMMENT / collation are deliberately excluded (not used in a
-- way that affects this codebase — see SPEC §3).

\pset footer off

-- 1. EXTENSIONS (by name; version omitted — same cluster, and version is not
--    part of the logical schema the migrations define).
SELECT 'EXT'::text AS k, extname AS a, ''::text AS b, ''::text AS c
FROM pg_extension
WHERE extname <> 'plpgsql'
ORDER BY extname;

-- 2. COLUMNS: per (table, column) → (type, nullable, default, generated, genexpr).
--    Order-independent (sorted by table, column — NOT attnum).
--    format_type carries precision/typmod (varchar(n), numeric(p,s), vector(768),
--    halfvec(N)). Defaults & generation exprs via pg_get_expr (canonical render).
SELECT
  'COL'::text AS k,
  c.relname || '.' || a.attname AS a,
  format_type(a.atttypid, a.atttypmod)
    || ' null=' || (NOT a.attnotnull)::text
    || ' gen=' || a.attgenerated::text AS b,
  regexp_replace(COALESCE(pg_get_expr(ad.adbin, ad.adrelid), ''), E'\n', ' ⏎ ', 'g') AS c
FROM pg_attribute a
JOIN pg_class c ON c.oid = a.attrelid
JOIN pg_namespace n ON n.oid = c.relnamespace
LEFT JOIN pg_attrdef ad ON ad.adrelid = a.attrelid AND ad.adnum = a.attnum
WHERE n.nspname = 'public'
  AND c.relkind IN ('r', 'p')           -- ordinary + partitioned tables
  AND a.attnum > 0
  AND NOT a.attisdropped
  AND c.relname <> '_sqlx_migrations'    -- migration bookkeeping is not schema
ORDER BY a.attnum * 0, c.relname, a.attname;

-- 3. CONSTRAINTS: per table, contype + canonical def (name NOT emitted — the def
--    from pg_get_constraintdef excludes the auto-name; captures FK target+cols+
--    ON UPDATE/DELETE/DEFERRABLE, CHECK expr, PK/UNIQUE col-sets).
SELECT
  'CON'::text AS k,
  c.relname AS a,
  con.contype::text AS b,
  regexp_replace(pg_get_constraintdef(con.oid, true), E'\n', ' ⏎ ', 'g') AS c
FROM pg_constraint con
JOIN pg_class c ON c.oid = con.conrelid
JOIN pg_namespace n ON n.oid = c.relnamespace
WHERE n.nspname = 'public'
  AND c.relname <> '_sqlx_migrations'
ORDER BY c.relname, con.contype, pg_get_constraintdef(con.oid, true);

-- 4. INDEXES: name-normalized pg_get_indexdef (keeps opclass e.g.
--    halfvec_cosine_ops, partial predicate, access method). The index name is
--    stripped so auto-name differences never false-fail; constraint-backed
--    indexes are excluded (already covered as CONstraints above) to avoid
--    double-counting.
SELECT
  'IDX'::text AS k,
  ci.relname_table AS a,
  ''::text AS b,
  regexp_replace(
    pg_get_indexdef(ci.indexrelid),
    'INDEX .+? ON',            -- drop "INDEX <name> ON"
    'INDEX ON'
  ) AS c
FROM (
  SELECT i.indexrelid, t.relname AS relname_table
  FROM pg_index i
  JOIN pg_class idx ON idx.oid = i.indexrelid
  JOIN pg_class t ON t.oid = i.indrelid
  JOIN pg_namespace n ON n.oid = t.relnamespace
  WHERE n.nspname = 'public'
    AND t.relname <> '_sqlx_migrations'
    AND NOT EXISTS (
      SELECT 1 FROM pg_constraint con
      WHERE con.conindid = i.indexrelid
    )
) ci
ORDER BY ci.relname_table,
  regexp_replace(pg_get_indexdef(ci.indexrelid), 'INDEX .+? ON', 'INDEX ON');

-- 5. ENUMS: name + ordered label list.
SELECT
  'ENUM'::text AS k,
  t.typname AS a,
  ''::text AS b,
  string_agg(e.enumlabel, ',' ORDER BY e.enumsortorder) AS c
FROM pg_type t
JOIN pg_enum e ON e.enumtypid = t.oid
JOIN pg_namespace n ON n.oid = t.typnamespace
WHERE n.nspname = 'public'
GROUP BY t.typname
ORDER BY t.typname;

-- 6. SEQUENCES: name + type + increment/min/max/start/cycle (definition, not
--    just name — auto-named identity/serial sequences compared by shape).
SELECT
  'SEQ'::text AS k,
  s.seqname AS a,
  s.data_type AS b,
  'inc=' || s.increment || ' min=' || s.minimum_value || ' max=' || s.maximum_value
    || ' start=' || s.start_value || ' cycle=' || s.cycle::text AS c
FROM (
  SELECT
    c.relname AS seqname,
    format_type(seq.seqtypid, NULL) AS data_type,
    seq.seqincrement AS increment,
    seq.seqmin AS minimum_value,
    seq.seqmax AS maximum_value,
    seq.seqstart AS start_value,
    seq.seqcycle AS cycle
  FROM pg_sequence seq
  JOIN pg_class c ON c.oid = seq.seqrelid
  JOIN pg_namespace n ON n.oid = c.relnamespace
  WHERE n.nspname = 'public'
) s
ORDER BY s.seqname;

-- 7. FUNCTIONS: user-defined functions in public, by canonical definition.
--    Exclude extension-owned functions (pgvector installs its funcs/aggregates
--    into public — those belong to the extension, not the migration schema) and
--    restrict to plain functions (prokind='f') so aggregates/window funcs don't
--    trip pg_get_functiondef.
SELECT
  'FUNC'::text AS k,
  p.proname AS a,
  pg_get_function_identity_arguments(p.oid) AS b,
  regexp_replace(pg_get_functiondef(p.oid), E'\n', ' ⏎ ', 'g') AS c
FROM pg_proc p
JOIN pg_namespace n ON n.oid = p.pronamespace
WHERE n.nspname = 'public'
  AND p.prokind = 'f'
  AND NOT EXISTS (
    SELECT 1 FROM pg_depend d
    WHERE d.classid = 'pg_proc'::regclass AND d.objid = p.oid AND d.deptype = 'e'
  )
ORDER BY p.proname, pg_get_function_identity_arguments(p.oid);

-- 8. TRIGGERS: non-internal triggers by canonical definition.
SELECT
  'TRIG'::text AS k,
  c.relname AS a,
  t.tgname AS b,
  regexp_replace(pg_get_triggerdef(t.oid), E'\n', ' ⏎ ', 'g') AS c
FROM pg_trigger t
JOIN pg_class c ON c.oid = t.tgrelid
JOIN pg_namespace n ON n.oid = c.relnamespace
WHERE n.nspname = 'public'
  AND NOT t.tgisinternal
ORDER BY c.relname, t.tgname;
