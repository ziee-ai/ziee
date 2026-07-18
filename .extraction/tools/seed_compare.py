#!/usr/bin/env python3
"""EA-seed equivalence check (SPEC §3 EA-seed).

Compares the whole-DB seed image of two freshly-migrated DBs (a fresh-migrated DB
holds ONLY seed rows). Per table:
  (1) drop volatile columns (created_at/updated_at/*_at timestamps + surrogate
      gen_random_uuid PKs);
  (2) resolve FK columns THROUGH the referenced row's business key (so a
      differently-generated-but-equivalent uuid graph matches);
  (3) element-sort set-valued array columns (order not significant);
  (4) key each row by its business key (natural cols) — NOT the surrogate uuid —
      and compare per key; tables without a business key fall back to a
      normalized-tuple multiset compare (singletons/join rows).
Missing / extra / business-key-mismatched / content-differing row => FAIL.

Usage:
  seed_compare.py <baseline_url> <candidate_url>   # compare two live DBs
  seed_compare.py --emit <url>                       # emit the canonical seed
      image (business-key-normalized text) to stdout — the self-contained,
      re-runnable anchor committed as .extraction/baseline/seed.canonical.txt
      (the numeric baseline DB is not reproducible from the tree post-squash).
Exit 0 = equivalent/emitted, 1 = differences, 2 = usage/error.
"""
import sys, subprocess, json
from collections import defaultdict

# Natural business keys per table (surrogate uuid PKs are NOT keys).
BUSINESS_KEY = {
    "groups": ["name"],
    "auth_providers": ["name"],
    "llm_providers": ["name"],
    "llm_repositories": ["name"],
    "assistants": ["name"],
    "mcp_servers": ["name"],
}
# Referenced business key used when resolving an FK that points at a table.
REF_KEY = dict(BUSINESS_KEY)

def psql_json(url, sql):
    out = subprocess.run(
        ["psql", url, "-X", "-t", "-A", "-c",
         f"SELECT coalesce(json_agg(t),'[]') FROM ({sql}) t"],
        capture_output=True, text=True)
    if out.returncode != 0:
        print("psql error:", out.stderr, file=sys.stderr); sys.exit(2)
    return json.loads(out.stdout.strip() or "[]")

def seeded_tables(url):
    rows = psql_json(url,
        "SELECT tablename FROM pg_tables WHERE schemaname='public' "
        "AND tablename<>'_sqlx_migrations'")
    tbls = []
    for r in rows:
        t = r["tablename"]
        c = psql_json(url, f"SELECT count(*) n FROM \"{t}\"")[0]["n"]
        if int(c) > 0:
            tbls.append(t)
    return set(tbls)

def columns(url, t):
    return psql_json(url,
        "SELECT column_name, data_type FROM information_schema.columns "
        f"WHERE table_schema='public' AND table_name='{t}' ORDER BY ordinal_position")

def surrogate_pk_cols(url, t):
    # uuid PK column(s) whose default is gen_random_uuid => volatile surrogate.
    rows = psql_json(url, f"""
        SELECT a.attname AS c
        FROM pg_index i JOIN pg_attribute a ON a.attrelid=i.indrelid AND a.attnum=ANY(i.indkey)
        JOIN pg_attrdef ad ON ad.adrelid=a.attrelid AND ad.adnum=a.attnum
        WHERE i.indrelid='public.{t}'::regclass AND i.indisprimary
          AND format_type(a.atttypid,a.atttypmod)='uuid'
          AND pg_get_expr(ad.adbin,ad.adrelid) ILIKE '%gen_random_uuid%'""")
    return {r["c"] for r in rows}

def fk_map(url, t):
    # {local_col -> referenced_table} for single-column FKs.
    rows = psql_json(url, f"""
        SELECT att.attname AS local_col, cl.relname AS ref_table
        FROM pg_constraint con
        JOIN pg_class c ON c.oid=con.conrelid
        JOIN pg_class cl ON cl.oid=con.confrelid
        JOIN pg_attribute att ON att.attrelid=con.conrelid AND att.attnum=con.conkey[1]
        WHERE con.contype='f' AND c.relname='{t}' AND array_length(con.conkey,1)=1""")
    return {r["local_col"]: r["ref_table"] for r in rows}

def volatile_cols(url, t):
    v = set()
    for col in columns(url, t):
        n, dt = col["column_name"], col["data_type"]
        if n in ("created_at", "updated_at"): v.add(n)
        elif n.endswith("_at") and "timestamp" in dt: v.add(n)
        elif n.endswith("_encrypted"): v.add(n)  # non-deterministic ciphertext if set
    v |= surrogate_pk_cols(url, t)
    return v

def load_rows(url, t):
    return psql_json(url, f'SELECT * FROM "{t}"')

def build_ref_index(url, tables):
    """For each table, map uuid PK value -> its business-key tuple (for FK resolution)."""
    idx = {}
    for t in tables:
        key = REF_KEY.get(t)
        rows = load_rows(url, t)
        # find the PK uuid col name
        pk = surrogate_pk_cols(url, t)
        pkcol = next(iter(pk)) if pk else "id"
        m = {}
        for r in rows:
            if pkcol in r and key:
                m[r[pkcol]] = tuple(r[k] for k in key)
            elif pkcol in r:
                m[r[pkcol]] = None  # no business key -> can't resolve; leave uuid
        idx[t] = (pkcol, m)
    return idx

def normalize(url, t, rows, refidx):
    vol = volatile_cols(url, t)
    fks = fk_map(url, t)
    coltypes = {c["column_name"]: c["data_type"] for c in columns(url, t)}
    norm = []
    for r in rows:
        d = {}
        for k, val in r.items():
            if k in vol:
                continue
            if k in fks and val is not None:
                ref_t = fks[k]
                _, m = refidx.get(ref_t, ("", {}))
                resolved = m.get(val)
                d[k] = ("REF:" + ref_t + ":" + repr(resolved)) if resolved is not None else val
                continue
            if coltypes.get(k) == "ARRAY" and isinstance(val, list):
                d[k] = sorted(val, key=lambda x: (x is None, str(x)))
                continue
            d[k] = val
        norm.append(d)
    return norm, BUSINESS_KEY.get(t)

def canon(d):
    return json.dumps(d, sort_keys=True, default=str)

def emit_canonical(url):
    """Deterministic business-key-normalized whole-DB seed image (one line per
    row: `table\\tcanon-json`), globally sorted. Self-contained (FK resolution +
    volatile-col detection query only THIS db)."""
    tbls = seeded_tables(url)
    ridx = build_ref_index(url, tbls)
    lines = []
    for t in sorted(tbls):
        norm, _ = normalize(url, t, load_rows(url, t), ridx)
        for d in norm:
            lines.append(t + "\t" + canon(d))
    lines.sort()
    return "\n".join(lines) + "\n"

def main():
    if len(sys.argv) == 3 and sys.argv[1] == "--emit":
        sys.stdout.write(emit_canonical(sys.argv[2])); return
    if len(sys.argv) != 3:
        print("usage: seed_compare.py <baseline_url> <candidate_url> | --emit <url>"); sys.exit(2)
    base, cand = sys.argv[1], sys.argv[2]
    tb, tc = seeded_tables(base), seeded_tables(cand)
    fails = []
    if tb != tc:
        only_b, only_c = tb - tc, tc - tb
        if only_b: fails.append(f"tables seeded only in baseline: {sorted(only_b)}")
        if only_c: fails.append(f"tables seeded only in candidate: {sorted(only_c)}")
    ridx_b = build_ref_index(base, tb)
    ridx_c = build_ref_index(cand, tc)
    for t in sorted(tb & tc):
        nb, key = normalize(base, t, load_rows(base, t), ridx_b)
        nc, _ = normalize(cand, t, load_rows(cand, t), ridx_c)
        if key:  # business-key keyed compare
            def bucket(rows):
                m = {}
                for d in rows:
                    kk = tuple(d.get(k) for k in key)
                    m.setdefault(kk, []).append(canon(d))
                return m
            bb, bc = bucket(nb), bucket(nc)
            for kk in set(bb) | set(bc):
                if kk not in bb: fails.append(f"{t}: key {kk} only in candidate")
                elif kk not in bc: fails.append(f"{t}: key {kk} only in baseline")
                elif sorted(bb[kk]) != sorted(bc[kk]):
                    fails.append(f"{t}: key {kk} content differs\n   base={sorted(bb[kk])}\n   cand={sorted(bc[kk])}")
        else:  # multiset compare
            mb, mc = sorted(canon(d) for d in nb), sorted(canon(d) for d in nc)
            if mb != mc:
                mbset, mcset = defaultdict(int), defaultdict(int)
                for x in mb: mbset[x]+=1
                for x in mc: mcset[x]+=1
                for x in set(mbset)|set(mcset):
                    if mbset[x]!=mcset[x]:
                        fails.append(f"{t}: row multiset differs (base×{mbset[x]} cand×{mcset[x]}): {x}")
    if fails:
        print("EA-seed: DIFFERENCES")
        for f in fails: print("  -", f)
        sys.exit(1)
    print(f"EA-seed: EQUIVALENT ✓ ({len(tb & tc)} seeded tables compared)")

if __name__ == "__main__":
    main()
