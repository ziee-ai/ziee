-- ============================================================================
-- Showcase chat conversation seed
-- ============================================================================
-- ONE long, deterministic conversation that exercises EVERY renderable chat
-- block type, for visual QA of the chat UI. Import is idempotent (fixed UUIDs +
-- ON CONFLICT DO NOTHING), so re-running just no-ops.
--
-- Run via load.sh (which resolves the owner user + copies file bytes into the
-- file store). Do not run this file by hand unless you pass:
--     psql -v owner="'<admin-user-uuid>'" -f showcase.sql
--
-- Schema references (investigated, not guessed):
--   conversations / branches / messages / branch_messages / message_contents
--     -- migrations 9-13, 23 (messages.model_id), 124 (uq message seq)
--   message_contents.content = JSONB MessageContentData, tagged by "type":
--     text | thinking | image | file_attachment | tool_use | tool_result |
--     elicitation_request   (chat/file/mcp extension variant enums)
--   files -- migration 14 (+34 created_by); on-disk originals placed by load.sh
--   mcp_tool_calls -- migration 105 (the "Calls" tab / tool-call history)
--   projects / project_files -- migrations 51-53
--
-- Built-in MCP server UUIDs are deterministic (uuid_v5(NAMESPACE_URL,
--   "<name>.ziee.internal")) and exist as mcp_servers rows once the server has
--   booted against this DB. tool_use.server_id references them so the renderer
--   resolves a server name.
--
-- HOW TO EXTEND: every section below ends with an "add more here" anchor.
-- ============================================================================

\set ON_ERROR_STOP on

-- ---------------------------------------------------------------------------
-- Fixed IDs (change nothing here on re-import — that is the point).
-- ---------------------------------------------------------------------------
--  owner              :owner   (psql var, resolved by load.sh)
--  project            90000000-0000-0000-0000-000000000001
--  conversation       11111111-1111-1111-1111-111111111111
--  branch             22222222-2222-2222-2222-222222222222
--  messages           30000000-0000-0000-0000-0000000000NN
--  built-in servers   (uuid_v5)  code-sandbox b4d4e17b-55eb-56ce-9bc5-cbc03fd597fd
--                                 web_search   d1a783dc-631e-570b-aba6-fee5497728b2
--                                 lit_search   5bf27612-ac1b-5141-985b-e2e8ac36ca2d
--                                 memory       16e2eeb0-46ed-5588-af8a-e973349f99a1
--                                 files        ca77f284-c0c3-51e0-ae83-8e34daa081f6
--                                 citations    011e52cb-2d06-5e6b-8f4c-41076519f167
--                                 control      d878787e-aa48-5f16-a31f-673052083f34
--                                 tool_result  62c47165-bcf4-5daf-b778-8eff985ac943
--  files (originals placed by load.sh under originals/<owner>/<id>.<ext>)
--                     chart.png     f1000000-0000-0000-0000-000000000001
--                     photo.jpg     f1000000-0000-0000-0000-000000000002
--                     workbook.xlsx f1000000-0000-0000-0000-000000000003
--                     data.csv      f1000000-0000-0000-0000-000000000004
--                     report.pdf    f1000000-0000-0000-0000-000000000005
--                     script.py     f1000000-0000-0000-0000-000000000006
--                     notes.md      f1000000-0000-0000-0000-000000000007
--                     large.txt     f1000000-0000-0000-0000-000000000008

BEGIN;

-- ===========================================================================
-- 0. FILES  (rows only; bytes copied to disk by load.sh)
--    checksum = sha256 hex (matches the module's calculate_checksum).
--    Migration 93 added file_versions: every file needs a v1 head row with
--    id == file_id == blob_version_id (so the on-disk originals/ path resolves),
--    and files.current_version_id = file_id. The head FK is DEFERRABLE, so both
--    inserts live in this one BEGIN/COMMIT (checked at COMMIT).
-- ===========================================================================
INSERT INTO files (id, user_id, filename, file_size, mime_type, checksum, created_by, current_version_id) VALUES
  ('f1000000-0000-0000-0000-000000000001', :'owner', 'chart.png',     6381,  'image/png',        '379cf6ce7d18a41a347600c4f04b2abf267f076e62fe59550746c2f38d663e64', 'llm',  'f1000000-0000-0000-0000-000000000001'),
  ('f1000000-0000-0000-0000-000000000002', :'owner', 'photo.jpg',     8309,  'image/jpeg',       '4f513b3e982d7d061652dd1afcb4d456fffae2174d80bec90f35319f0f9808b7', 'user', 'f1000000-0000-0000-0000-000000000002'),
  ('f1000000-0000-0000-0000-000000000003', :'owner', 'workbook.xlsx', 6641,  'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet', '392d315220993aa6f35e510235b7c8d27da08ae0986d85d6a328d62e9e87b472', 'user', 'f1000000-0000-0000-0000-000000000003'),
  ('f1000000-0000-0000-0000-000000000004', :'owner', 'data.csv',      133,   'text/csv',         '24b7ad6666ae8408548fc598bbf8a2aac2b555ec9d3b7a753772706bca62add5', 'user', 'f1000000-0000-0000-0000-000000000004'),
  ('f1000000-0000-0000-0000-000000000005', :'owner', 'report.pdf',    631,   'application/pdf',  '265e064806529bb6d8f8154a53183dfaa404da867ca37b710282b91b16a144cb', 'user', 'f1000000-0000-0000-0000-000000000005'),
  ('f1000000-0000-0000-0000-000000000006', :'owner', 'script.py',     261,   'text/x-python',    'd04d007fd6bd8aaaf7ed89f00811430ff5ca1a3a6e80dc0cb1615f78f1ebce57', 'user', 'f1000000-0000-0000-0000-000000000006'),
  ('f1000000-0000-0000-0000-000000000007', :'owner', 'notes.md',      292,   'text/markdown',    'c73434bbe224db0d228d8457dd695a83ac7c72febe806a179a75154acf955217', 'user', 'f1000000-0000-0000-0000-000000000007'),
  ('f1000000-0000-0000-0000-000000000008', :'owner', 'large.txt',     86400, 'text/plain',       'e5f6bc1fcec698fc465f6c45d4af098c1b1a734782770f3a9d7e61daaa8cc8a9', 'user', 'f1000000-0000-0000-0000-000000000008')
ON CONFLICT (id) DO NOTHING;

-- v1 head version per file (id = file_id = blob_version_id; is_head).
INSERT INTO file_versions (id, file_id, version, is_head, blob_version_id, file_size, mime_type, checksum, created_by)
SELECT id, id, 1, true, id, file_size, mime_type, checksum, created_by
FROM files WHERE id IN (
  'f1000000-0000-0000-0000-000000000001','f1000000-0000-0000-0000-000000000002',
  'f1000000-0000-0000-0000-000000000003','f1000000-0000-0000-0000-000000000004',
  'f1000000-0000-0000-0000-000000000005','f1000000-0000-0000-0000-000000000006',
  'f1000000-0000-0000-0000-000000000007','f1000000-0000-0000-0000-000000000008')
ON CONFLICT (id) DO NOTHING;
-- -- add more files here (also add bytes to files/ + FILE_MAP in load.sh) --

-- ===========================================================================
-- 1. PROJECT + membership (exercises project files + conversation.project_id)
-- ===========================================================================
INSERT INTO projects (id, user_id, name, description, instructions) VALUES
  ('90000000-0000-0000-0000-000000000001', :'owner', 'UI Showcase',
   'Demo project that hosts the showcase conversation.',
   'You are a rendering test fixture. Prefer rich, varied formatting.')
ON CONFLICT (id) DO NOTHING;

INSERT INTO project_files (project_id, file_id) VALUES
  ('90000000-0000-0000-0000-000000000001', 'f1000000-0000-0000-0000-000000000007'),
  ('90000000-0000-0000-0000-000000000001', 'f1000000-0000-0000-0000-000000000004')
ON CONFLICT DO NOTHING;

-- An EXTERNAL (user-added, non-built-in) MCP server so the tool_use card for an
-- external tool resolves a display name (the renderer falls back to the raw
-- server_id if absent — this exercises the resolved path + is_built_in=false).
INSERT INTO mcp_servers (id, user_id, name, display_name, is_built_in, is_system, transport_type, url) VALUES
  ('e0000000-0000-0000-0000-0000000000e1', :'owner', 'weather-api', 'Weather API (external)', false, false, 'http', 'https://example.com/mcp')
ON CONFLICT (id) DO NOTHING;

-- ===========================================================================
-- 2. CONVERSATION + BRANCH (circular FK: conv, then branch, then set active)
-- ===========================================================================
INSERT INTO conversations (id, user_id, title, created_at) VALUES
  ('11111111-1111-1111-1111-111111111111', :'owner',
   'Rendering Showcase — every block type',
   TIMESTAMPTZ '2026-07-01 12:00:00+00')
ON CONFLICT (id) DO NOTHING;

-- conversation -> project link (migration 73 moved this off conversations into
-- the project_conversations join; one project per conversation).
INSERT INTO project_conversations (conversation_id, project_id) VALUES
  ('11111111-1111-1111-1111-111111111111', '90000000-0000-0000-0000-000000000001')
ON CONFLICT (conversation_id) DO NOTHING;

INSERT INTO branches (id, conversation_id, created_at) VALUES
  ('22222222-2222-2222-2222-222222222222', '11111111-1111-1111-1111-111111111111',
   TIMESTAMPTZ '2026-07-01 12:00:00+00')
ON CONFLICT (id) DO NOTHING;

UPDATE conversations SET active_branch_id = '22222222-2222-2222-2222-222222222222'
  WHERE id = '11111111-1111-1111-1111-111111111111';

COMMIT;

-- ===========================================================================
-- 3. MESSAGES + CONTENT BLOCKS
-- ===========================================================================
-- Helper convention used below:
--   * messages.originated_from_id = its own id (new, non-edited message).
--   * branch_messages.created_at drives render order: base + N seconds.
--   * every content row uses ON CONFLICT (message_id, sequence_order).
-- ===========================================================================
BEGIN;

-- ---- tiny inline helpers via a temp function (dropped at COMMIT) ----------
-- msg(): insert a message + its branch link at ordinal `n`.
CREATE OR REPLACE FUNCTION pg_temp.msg(mid uuid, mrole text, n numeric) RETURNS void AS $fn$
BEGIN
  INSERT INTO messages (id, role, originated_from_id, edit_count, created_at)
    VALUES (mid, mrole, mid, 0, TIMESTAMPTZ '2026-07-01 12:00:00+00' + (n || ' seconds')::interval)
    ON CONFLICT (id) DO NOTHING;
  INSERT INTO branch_messages (branch_id, message_id, is_clone, created_at)
    VALUES ('22222222-2222-2222-2222-222222222222', mid, false,
            TIMESTAMPTZ '2026-07-01 12:00:00+00' + (n || ' seconds')::interval)
    ON CONFLICT (branch_id, message_id) DO NOTHING;
END;
$fn$ LANGUAGE plpgsql;

-- blk(): insert a content block.
CREATE OR REPLACE FUNCTION pg_temp.blk(mid uuid, seq int, ctype text, body jsonb) RETURNS void AS $fn$
BEGIN
  INSERT INTO message_contents (message_id, content_type, content, sequence_order)
    VALUES (mid, ctype, body, seq)
    ON CONFLICT (message_id, sequence_order) DO NOTHING;
END;
$fn$ LANGUAGE plpgsql;

-- ###########################################################################
-- SECTION A — MARKDOWN
-- ###########################################################################

-- Turn 1: user asks; assistant returns the full markdown formatting kitchen sink
SELECT pg_temp.msg('30000000-0000-0000-0000-000000000001', 'user', 1);
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000001', 0, 'text',
  jsonb_build_object('type','text','text',
    $u$Show me **every** markdown formatting feature you can render.$u$));

SELECT pg_temp.msg('30000000-0000-0000-0000-000000000002', 'assistant', 2);
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000002', 0, 'text',
  jsonb_build_object('type','text','text', $md$# Heading 1
## Heading 2
### Heading 3
#### Heading 4
##### Heading 5
###### Heading 6

Inline styles: **bold**, *italic*, ***bold italic***, ~~strikethrough~~, `inline code`, and a [link](https://example.com).

> Blockquote level 1
>> Nested blockquote level 2
>>> Level 3 with `code` inside.

Unordered list:
- Item one
- Item two
  - Nested item 2a
  - Nested item 2b
    - Deeper 2b-i

Ordered list:
1. First
2. Second
   1. Second-a
   2. Second-b

Task list:
- [x] Done task
- [ ] Pending task
- [ ] Another pending

A simple table:

| Name | Role | Score |
|------|------|-------|
| Ada  | Lead | 99    |
| Bob  | Dev  | 87    |

A wide / complex table:

| ID | Metric | Q1 | Q2 | Q3 | Q4 | Trend | Owner | Notes |
|----|--------|----|----|----|----|-------|-------|-------|
| 1  | Revenue| 120| 200| 160| 280| up    | NA    | strong close |
| 2  | Costs  | 74 | 71 | 69 | 66 | down  | Ops   | efficiency gains |
| 3  | Margin | 46 | 129| 91 | 214| up    | Fin   | *see footnote[^1]* |

---

Inline image (may 401 offline — exercises the `<img>` layout):
![alt text](https://raw.githubusercontent.com/phibya/ziee-chat/main/does-not-exist.png)

Here is a footnote reference[^1] and a second[^2].

[^1]: This is the first footnote definition.
[^2]: The second footnote, with `code` and a [link](https://example.com).
$md$));
-- -- add more markdown-feature blocks here --

-- Turn 2: fenced code blocks in many languages + a deliberately long one
SELECT pg_temp.msg('30000000-0000-0000-0000-000000000003', 'user', 3);
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000003', 0, 'text',
  jsonb_build_object('type','text','text',
    $u$Now show fenced code blocks in many languages, plus one very long block.$u$));

SELECT pg_temp.msg('30000000-0000-0000-0000-000000000004', 'assistant', 4);
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000004', 0, 'text',
  jsonb_build_object('type','text','text', $md$Code in several languages:

```rust
fn main() {
    let xs: Vec<i32> = (0..5).map(|x| x * x).collect();
    println!("{:?}", xs);
}
```

```python
def fib(n):
    a, b = 0, 1
    for _ in range(n):
        a, b = b, a + b
    return a
```

```typescript
const greet = (name: string): string => `Hello, ${name}!`
console.log(greet("world"))
```

```sql
SELECT u.id, count(*) AS n
FROM users u JOIN messages m ON m.user_id = u.id
GROUP BY u.id ORDER BY n DESC;
```

```bash
#!/usr/bin/env bash
set -euo pipefail
for f in *.txt; do wc -l "$f"; done
```

```json
{ "ok": true, "items": [1, 2, 3], "nested": { "a": null } }
```

```yaml
service:
  name: ziee
  ports: [5173, 3000]
  flags: { debug: true }
```

```diff
- const old = 1
+ const updated = 2
```

```html
<button class="btn" onclick="run()">Run</button>
```
$md$));
-- Long code block (scroll test) — kept in its own block.
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000004', 1, 'text',
  jsonb_build_object('type','text','text', $md$A long code block to test vertical scrolling / max-height:

```python
# 60+ lines of filler to force a scroll region
def line_000(): return 0
def line_001(): return 1
def line_002(): return 2
def line_003(): return 3
def line_004(): return 4
def line_005(): return 5
def line_006(): return 6
def line_007(): return 7
def line_008(): return 8
def line_009(): return 9
def line_010(): return 10
def line_011(): return 11
def line_012(): return 12
def line_013(): return 13
def line_014(): return 14
def line_015(): return 15
def line_016(): return 16
def line_017(): return 17
def line_018(): return 18
def line_019(): return 19
def line_020(): return 20
def line_021(): return 21
def line_022(): return 22
def line_023(): return 23
def line_024(): return 24
def line_025(): return 25
def line_026(): return 26
def line_027(): return 27
def line_028(): return 28
def line_029(): return 29
def line_030(): return 30
def line_031(): return 31
def line_032(): return 32
def line_033(): return 33
def line_034(): return 34
def line_035(): return 35
def line_036(): return 36
def line_037(): return 37
def line_038(): return 38
def line_039(): return 39
def line_040(): return 40
if __name__ == "__main__":
    print(sum(f() for f in [line_000, line_001, line_002, line_003]))
```
$md$));

-- Turn 3: math + mermaid + long prose (streamdown supports remark-math + mermaid)
SELECT pg_temp.msg('30000000-0000-0000-0000-000000000005', 'user', 5);
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000005', 0, 'text',
  jsonb_build_object('type','text','text', $u$Show LaTeX math, a Mermaid diagram, and a long prose block.$u$));

SELECT pg_temp.msg('30000000-0000-0000-0000-000000000006', 'assistant', 6);
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000006', 0, 'text',
  jsonb_build_object('type','text','text', $md$Inline math: the mass–energy relation is $E = mc^2$, and Euler's identity $e^{i\pi} + 1 = 0$.

Block math:

$$
\int_{-\infty}^{\infty} e^{-x^2}\,dx = \sqrt{\pi}
$$

$$
\frac{\partial}{\partial t}\Psi = \hat{H}\Psi
$$

A Mermaid flowchart:

```mermaid
flowchart TD
    A[User message] --> B{Tool needed?}
    B -->|yes| C[tool_use]
    C --> D[tool_result]
    D --> E[Assistant reply]
    B -->|no| E
```

A Mermaid sequence diagram:

```mermaid
sequenceDiagram
    participant U as User
    participant S as Server
    participant M as MCP
    U->>S: send message
    S->>M: call_tool
    M-->>S: tool_result
    S-->>U: streamed reply
```
$md$));
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000006', 1, 'text',
  jsonb_build_object('type','text','text', $md$**Long prose block (scroll / wrap test).** Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.

Sed ut perspiciatis unde omnis iste natus error sit voluptatem accusantium doloremque laudantium, totam rem aperiam, eaque ipsa quae ab illo inventore veritatis et quasi architecto beatae vitae dicta sunt explicabo. Nemo enim ipsam voluptatem quia voluptas sit aspernatur aut odit aut fugit, sed quia consequuntur magni dolores eos qui ratione voluptatem sequi nesciunt.

Neque porro quisquam est, qui dolorem ipsum quia dolor sit amet, consectetur, adipisci velit, sed quia non numquam eius modi tempora incidunt ut labore et dolore magnam aliquam quaerat voluptatem. Ut enim ad minima veniam, quis nostrum exercitationem ullam corporis suscipit laboriosam, nisi ut aliquid ex ea commodi consequatur.$md$));

-- Turn 3b: MARKDOWN EDGE CASES — the exhaustive pass. Fractional ordinals slot
-- these right after the markdown section above. Grouped into labelled blocks so
-- a reviewer can see, per group, exactly which constructs render.
SELECT pg_temp.msg('30000000-0000-0000-0000-000000000040', 'user', 6.1);
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000040', 0, 'text',
  jsonb_build_object('type','text','text', $u$Now the edge cases — cover every markdown construct you can.$u$));

SELECT pg_temp.msg('30000000-0000-0000-0000-000000000041', 'assistant', 6.2);
-- Group 1: links + images (reference, autolink, bare, titles, image-as-link)
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000041', 0, 'text',
  jsonb_build_object('type','text','text', $md$### Links & images

Inline link with title: [hover me](https://example.com "Link title").
Reference-style link: [ref link][ref1] and collapsed [ref1][].
Autolink: <https://example.com/auto>. Bare URL (GFM autolink): https://example.com/bare.
Email autolink: <hello@example.com>.
Image with title: ![alt](https://example.com/x.png "Image title").
Image as a link: [![alt](https://example.com/thumb.png)](https://example.com).

[ref1]: https://example.com/reference
$md$));
-- Group 2: tables — alignment, in-cell formatting, empty cells, escaped pipes
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000041', 1, 'text',
  jsonb_build_object('type','text','text', $md$### Table alignment & cell formatting

| Left | Center | Right |
|:-----|:------:|------:|
| a    | **b**  | 1     |
| `c`  |        | 22    |
| ~~d~~| [e](https://example.com) | a \| b |

An empty cell is above (row 2, center). The last cell has an escaped pipe.
$md$));
-- Group 3: list variants (start-at-N, markers, tight/loose, nested blocks)
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000041', 2, 'text',
  jsonb_build_object('type','text','text', $md$### List variants

Ordered starting at 5:
5. five
6. six

Marker variants:
* star item
+ plus item
- dash item

Loose list (blank lines → wrapped in `<p>`):

- first

- second

List item containing a nested code block and a blockquote:

- item with code:
  ```
  nested fenced code inside a list item
  ```
  > and a blockquote inside the same item
$md$));
-- Group 4: inline breaks, escapes, entities, sub/sup, kbd, br
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000041', 3, 'text',
  jsonb_build_object('type','text','text', $md$### Inline breaks, escapes, entities, raw inline HTML

Hard line break (two trailing spaces) →
this is on a new line.
Backslash line break →\
also a new line.

Escaped characters: \*not italic\*, \# not a heading, \`not code\`, 1\. not a list.

HTML entities: &copy; &amp; &lt; &gt; &mdash; &hearts; &#8734;

Sub/super: H<sub>2</sub>O and E = mc<sup>2</sup>. Key: press <kbd>Ctrl</kbd>+<kbd>C</kbd>.
Highlight: <mark>marked text</mark>. Abbrev: <abbr title="HyperText Markup Language">HTML</abbr>.
Forced break next<br>and after the br.
$md$));
-- Group 5: GFM alerts / admonitions
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000041', 4, 'text',
  jsonb_build_object('type','text','text', $md$### GFM alerts (callouts)

> [!NOTE]
> Useful information the user should know.

> [!TIP]
> Helpful advice for doing things better.

> [!IMPORTANT]
> Key information the user needs.

> [!WARNING]
> Urgent info needing immediate attention.

> [!CAUTION]
> Advises about risks or negative outcomes.
$md$));
-- Group 6: raw HTML blocks (details/summary, html table)
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000041', 5, 'text',
  jsonb_build_object('type','text','text', $md$### Raw HTML blocks

<details>
<summary>Click to expand</summary>

Hidden content revealed on expand — including **markdown** inside.

</details>

<table>
  <tr><th>HTML th</th><th>Col 2</th></tr>
  <tr><td>cell</td><td>cell</td></tr>
</table>
$md$));

SELECT pg_temp.msg('30000000-0000-0000-0000-000000000042', 'assistant', 6.3);
-- Group 7: heading styles, code edge cases, hr variants
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000042', 0, 'text',
  jsonb_build_object('type','text','text', $md$Setext heading level 1
======================

Setext heading level 2
----------------------

Indented (4-space) code block:

    def indented():
        return "code via 4-space indent"

Fenced with `~~~` (so the body can contain triple backticks):

~~~markdown
Here is ```inline triple backtick``` inside a tilde fence.
~~~

Code span with backticks inside: `` a ` b ``.

Thematic break variants:

***

___
$md$));
-- Group 8: emoji, overflow/stress, consecutive code blocks
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000042', 1, 'text',
  jsonb_build_object('type','text','text', $md$### Emoji, overflow, consecutive blocks

Unicode emoji: 🎉 ✅ 🚀 — and shortcode form (if supported): :tada: :rocket:.

Very long unbroken token (horizontal-overflow test):
supercalifragilisticexpialidocioussupercalifragilisticexpialidocioussupercalifragilisticexpialidocious

Very long URL (wrap/overflow test):
https://example.com/very/long/path/that/keeps/going/and/going/segment/segment/segment/segment/segment/segment/segment?with=query&and=more&params=here

Two fenced blocks back-to-back:

```json
{"first": true}
```
```json
{"second": true}
```
$md$));
-- Group 9: math edge cases (aligned, matrix, text) + empty/mixed blockquote
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000042', 2, 'text',
  jsonb_build_object('type','text','text', $md$### Math edge cases & blockquote content

Aligned environment:

$$
\begin{aligned}
a &= b + c \\
  &= d + e + f
\end{aligned}
$$

Matrix:

$$
M = \begin{bmatrix} 1 & 0 \\ 0 & 1 \end{bmatrix}
$$

Inline with text macro: $f(x) = \text{sinc}(x) = \frac{\sin \pi x}{\pi x}$.

A blockquote containing a heading, a list, and code:

> #### Quoted heading
> - quoted list item
> - second item
>
> ```python
> print("code inside a blockquote")
> ```
$md$));
-- -- add more markdown edge-case blocks here --

-- ###########################################################################
-- SECTION B — THINKING BLOCKS
-- ###########################################################################
SELECT pg_temp.msg('30000000-0000-0000-0000-000000000007', 'user', 7);
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000007', 0, 'text',
  jsonb_build_object('type','text','text', $u$Think step by step, then answer: what is 17 * 23?$u$));

SELECT pg_temp.msg('30000000-0000-0000-0000-000000000008', 'assistant', 8);
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000008', 0, 'thinking',
  jsonb_build_object('type','thinking',
    'thinking', $th$Let me compute 17 * 23. 17 * 20 = 340, and 17 * 3 = 51, so 340 + 51 = 391. Double-check: 23 * 17 = 23 * 10 + 23 * 7 = 230 + 161 = 391. Consistent.$th$,
    'metadata', jsonb_build_object('token_count', 48)));
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000008', 1, 'text',
  jsonb_build_object('type','text','text', $md$**17 × 23 = 391.**$md$));

-- ###########################################################################
-- SECTION C — TOOL CALLS (built-in MCP servers)
--   Each assistant turn: optional text + tool_use + tool_result blocks, PLUS
--   a matching mcp_tool_calls row (see SECTION C-rows below) for the Calls tab.
-- ###########################################################################

-- C1: code_sandbox execute_command — success + stdout/stderr/exit + resource_link chart
SELECT pg_temp.msg('30000000-0000-0000-0000-000000000009', 'user', 9);
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000009', 0, 'text',
  jsonb_build_object('type','text','text', $u$Run a quick script that prints output and saves a chart.$u$));

SELECT pg_temp.msg('30000000-0000-0000-0000-00000000000a', 'assistant', 10);
SELECT pg_temp.blk('30000000-0000-0000-0000-00000000000a', 0, 'text',
  jsonb_build_object('type','text','text', $md$I'll run it in the sandbox.$md$));
SELECT pg_temp.blk('30000000-0000-0000-0000-00000000000a', 1, 'tool_use',
  jsonb_build_object('type','tool_use',
    'id','toolu_sandbox_1','name','execute_command',
    'server_id','b4d4e17b-55eb-56ce-9bc5-cbc03fd597fd',
    'input', jsonb_build_object('command','python plot.py','timeout_ms',30000)));
SELECT pg_temp.blk('30000000-0000-0000-0000-00000000000a', 2, 'tool_result',
  jsonb_build_object('type','tool_result',
    'tool_use_id','toolu_sandbox_1','name','execute_command',
    'server_id','b4d4e17b-55eb-56ce-9bc5-cbc03fd597fd',
    'content', $tr$exit_code: 0
--- stdout ---
Rendered 4 bars. Saved chart.png (640x400).
--- stderr ---
matplotlib: using Agg backend$tr$,
    'is_error', false,
    'structured_content', jsonb_build_object('exit_code',0,'stdout_bytes',54,'stderr_bytes',28,'duration_ms',812),
    'resource_links', jsonb_build_array(jsonb_build_object(
      'uri','/api/files/f1000000-0000-0000-0000-000000000001',
      'name','chart.png','mime_type','image/png','size',6381,
      'is_saved', true, 'file_id','f1000000-0000-0000-0000-000000000001'))));
SELECT pg_temp.blk('30000000-0000-0000-0000-00000000000a', 3, 'text',
  jsonb_build_object('type','text','text', $md$Done — the chart is attached above.$md$));

-- C2: web_search — typed structuredContent + text digest
SELECT pg_temp.msg('30000000-0000-0000-0000-00000000000b', 'user', 11);
SELECT pg_temp.blk('30000000-0000-0000-0000-00000000000b', 0, 'text',
  jsonb_build_object('type','text','text', $u$Search the web for "pgvector HNSW tuning".$u$));

SELECT pg_temp.msg('30000000-0000-0000-0000-00000000000c', 'assistant', 12);
SELECT pg_temp.blk('30000000-0000-0000-0000-00000000000c', 0, 'tool_use',
  jsonb_build_object('type','tool_use',
    'id','toolu_web_1','name','web_search',
    'server_id','d1a783dc-631e-570b-aba6-fee5497728b2',
    'input', jsonb_build_object('query','pgvector HNSW tuning','max_results',3)));
SELECT pg_temp.blk('30000000-0000-0000-0000-00000000000c', 1, 'tool_result',
  jsonb_build_object('type','tool_result',
    'tool_use_id','toolu_web_1','name','web_search',
    'server_id','d1a783dc-631e-570b-aba6-fee5497728b2',
    'content', $tr$Top results:
1. Tuning HNSW in pgvector — m, ef_construction, ef_search tradeoffs.
2. Benchmarking recall vs latency for vector indexes.
3. When to prefer IVFFlat over HNSW.$tr$,
    'is_error', false,
    'structured_content', jsonb_build_object(
      'provider','searxng',
      'results', jsonb_build_array(
        jsonb_build_object('title','Tuning HNSW in pgvector','url','https://example.com/hnsw','snippet','m and ef_construction control graph quality...'),
        jsonb_build_object('title','Recall vs latency','url','https://example.com/bench','snippet','higher ef_search improves recall...'),
        jsonb_build_object('title','IVFFlat vs HNSW','url','https://example.com/ivf','snippet','IVFFlat trains faster but...')))));

-- C3: web_search fetch_url
SELECT pg_temp.msg('30000000-0000-0000-0000-00000000000d', 'assistant', 13);
SELECT pg_temp.blk('30000000-0000-0000-0000-00000000000d', 0, 'tool_use',
  jsonb_build_object('type','tool_use',
    'id','toolu_fetch_1','name','fetch_url',
    'server_id','d1a783dc-631e-570b-aba6-fee5497728b2',
    'input', jsonb_build_object('url','https://example.com/hnsw')));
SELECT pg_temp.blk('30000000-0000-0000-0000-00000000000d', 1, 'tool_result',
  jsonb_build_object('type','tool_result',
    'tool_use_id','toolu_fetch_1','name','fetch_url',
    'server_id','d1a783dc-631e-570b-aba6-fee5497728b2',
    'content', $tr$# Tuning HNSW in pgvector

Set `m` (default 16) and `ef_construction` (default 64) at index build time.
Raise `hnsw.ef_search` at query time to trade latency for recall.$tr$,
    'is_error', false,
    'structured_content', jsonb_build_object('final_url','https://example.com/hnsw','char_count',180)));

-- C4: lit_search literature_search — structuredContent drives the screening card
SELECT pg_temp.msg('30000000-0000-0000-0000-00000000000e', 'user', 14);
SELECT pg_temp.blk('30000000-0000-0000-0000-00000000000e', 0, 'text',
  jsonb_build_object('type','text','text', $u$Find recent literature on "CRISPR base editing off-target".$u$));

SELECT pg_temp.msg('30000000-0000-0000-0000-00000000000f', 'assistant', 15);
SELECT pg_temp.blk('30000000-0000-0000-0000-00000000000f', 0, 'tool_use',
  jsonb_build_object('type','tool_use',
    'id','toolu_lit_1','name','literature_search',
    'server_id','5bf27612-ac1b-5141-985b-e2e8ac36ca2d',
    'input', jsonb_build_object('query','CRISPR base editing off-target','max_results',2,'year_from',2023)));
SELECT pg_temp.blk('30000000-0000-0000-0000-00000000000f', 1, 'tool_result',
  jsonb_build_object('type','tool_result',
    'tool_use_id','toolu_lit_1','name','literature_search',
    'server_id','5bf27612-ac1b-5141-985b-e2e8ac36ca2d',
    'content', $tr$2 records (europepmc, crossref). Completeness ~ moderate.$tr$,
    'is_error', false,
    -- Shape MUST match ui LiteratureResult / LiteratureRecord (types.ts) so the
    -- LiteratureToolResultCard screening card + "Open in screening" panel work:
    -- required per record: title, authors[], source, source_ids[], is_preprint, relevance.
    'structured_content', jsonb_build_object(
      'query','CRISPR base editing off-target',
      'identified', jsonb_build_object('europepmc',1,'crossref',1),
      'after_dedup', 2,
      'degraded_sources', jsonb_build_array(),
      'completeness', jsonb_build_object('estimate','moderate','method','source-overlap','caveat','adjunct to systematic search'),
      'records', jsonb_build_array(
        jsonb_build_object('doi','10.1000/beditor.2024','title','Base editing off-target profiling','abstract_text','We profile genome-wide off-target activity of adenine and cytosine base editors...','year',2024,'venue','Nature Methods','authors', jsonb_build_array('Lee','Gomez'),'source','europepmc','source_ids', jsonb_build_array('PMC123'),'is_preprint',false,'relevance',0.94,'cited_by_count',12),
        jsonb_build_object('doi','10.1000/crispr.2023','title','Genome-wide specificity of adenine base editors','year',2023,'venue','Cell','authors', jsonb_build_array('Ng'),'source','crossref','source_ids', jsonb_build_array('doi:10.1000/crispr.2023'),'is_preprint',false,'relevance',0.88,'cited_by_count',44)))));

-- C5: memory remember + recall (two calls in one assistant turn)
SELECT pg_temp.msg('30000000-0000-0000-0000-000000000010', 'user', 16);
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000010', 0, 'text',
  jsonb_build_object('type','text','text', $u$Remember that I prefer SI units, then recall my preferences.$u$));

SELECT pg_temp.msg('30000000-0000-0000-0000-000000000011', 'assistant', 17);
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000011', 0, 'tool_use',
  jsonb_build_object('type','tool_use','id','toolu_mem_1','name','remember',
    'server_id','16e2eeb0-46ed-5588-af8a-e973349f99a1',
    'input', jsonb_build_object('content','User prefers SI units.')));
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000011', 1, 'tool_result',
  jsonb_build_object('type','tool_result','tool_use_id','toolu_mem_1','name','remember',
    'server_id','16e2eeb0-46ed-5588-af8a-e973349f99a1',
    'content','Stored memory #1.','is_error',false));
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000011', 2, 'tool_use',
  jsonb_build_object('type','tool_use','id','toolu_mem_2','name','recall',
    'server_id','16e2eeb0-46ed-5588-af8a-e973349f99a1',
    'input', jsonb_build_object('query','preferences')));
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000011', 3, 'tool_result',
  jsonb_build_object('type','tool_result','tool_use_id','toolu_mem_2','name','recall',
    'server_id','16e2eeb0-46ed-5588-af8a-e973349f99a1',
    'content','1 memory: "User prefers SI units."','is_error',false,
    'structured_content', jsonb_build_object('memories', jsonb_build_array(
      jsonb_build_object('id',1,'text','User prefers SI units.','score',0.91)))));
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000011', 4, 'text',
  jsonb_build_object('type','text','text', $md$Noted — I'll use **SI units** going forward.$md$));

-- C6: citations format_citations — text result
SELECT pg_temp.msg('30000000-0000-0000-0000-000000000012', 'user', 18);
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000012', 0, 'text',
  jsonb_build_object('type','text','text', $u$Format the base-editing paper as APA.$u$));

SELECT pg_temp.msg('30000000-0000-0000-0000-000000000013', 'assistant', 19);
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000013', 0, 'tool_use',
  jsonb_build_object('type','tool_use','id','toolu_cit_1','name','format_citations',
    'server_id','011e52cb-2d06-5e6b-8f4c-41076519f167',
    'input', jsonb_build_object('style','apa','items', jsonb_build_array(jsonb_build_object('doi','10.1000/beditor.2024')))));
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000013', 1, 'tool_result',
  jsonb_build_object('type','tool_result','tool_use_id','toolu_cit_1','name','format_citations',
    'server_id','011e52cb-2d06-5e6b-8f4c-41076519f167',
    'content','Lee, & Gomez. (2024). Base editing off-target profiling.','is_error',false));

-- C7: control — LLM operating ziee's own REST API
SELECT pg_temp.msg('30000000-0000-0000-0000-000000000014', 'assistant', 20);
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000014', 0, 'tool_use',
  jsonb_build_object('type','tool_use','id','toolu_ctl_1','name','call_api',
    'server_id','d878787e-aa48-5f16-a31f-673052083f34',
    'input', jsonb_build_object('method','GET','path','/api/conversations?per_page=1')));
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000014', 1, 'tool_result',
  jsonb_build_object('type','tool_result','tool_use_id','toolu_ctl_1','name','call_api',
    'server_id','d878787e-aa48-5f16-a31f-673052083f34',
    'content','{ "total": 1, "items": [ { "title": "Rendering Showcase" } ] }','is_error',false,
    'structured_content', jsonb_build_object('status',200)));

-- C8: get_tool_result — paged recall of a prior result
SELECT pg_temp.msg('30000000-0000-0000-0000-000000000015', 'assistant', 21);
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000015', 0, 'tool_use',
  jsonb_build_object('type','tool_use','id','toolu_gtr_1','name','get_tool_result',
    'server_id','62c47165-bcf4-5daf-b778-8eff985ac943',
    'input', jsonb_build_object('tool_use_id','toolu_web_1','offset',0,'max_chars',200)));
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000015', 1, 'tool_result',
  jsonb_build_object('type','tool_result','tool_use_id','toolu_gtr_1','name','get_tool_result',
    'server_id','62c47165-bcf4-5daf-b778-8eff985ac943',
    'content','Top results:\n1. Tuning HNSW in pgvector...','is_error',false));

-- C9: FAILED tool result (is_error true; status failed in mcp_tool_calls)
SELECT pg_temp.msg('30000000-0000-0000-0000-000000000016', 'user', 22);
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000016', 0, 'text',
  jsonb_build_object('type','text','text', $u$Run a command that fails.$u$));

SELECT pg_temp.msg('30000000-0000-0000-0000-000000000017', 'assistant', 23);
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000017', 0, 'tool_use',
  jsonb_build_object('type','tool_use','id','toolu_sandbox_err','name','execute_command',
    'server_id','b4d4e17b-55eb-56ce-9bc5-cbc03fd597fd',
    'input', jsonb_build_object('command','cat /nope','timeout_ms',5000)));
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000017', 1, 'tool_result',
  jsonb_build_object('type','tool_result','tool_use_id','toolu_sandbox_err','name','execute_command',
    'server_id','b4d4e17b-55eb-56ce-9bc5-cbc03fd597fd',
    'content', $tr$exit_code: 1
--- stderr ---
cat: /nope: No such file or directory$tr$,
    'is_error', true,
    'structured_content', jsonb_build_object('exit_code',1)));

-- C10: CANCELLED tool call (renders + Calls tab shows cancelled)
SELECT pg_temp.msg('30000000-0000-0000-0000-000000000018', 'assistant', 24);
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000018', 0, 'tool_use',
  jsonb_build_object('type','tool_use','id','toolu_cancelled','name','execute_command',
    'server_id','b4d4e17b-55eb-56ce-9bc5-cbc03fd597fd',
    'input', jsonb_build_object('command','sleep 999','timeout_ms',1000)));
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000018', 1, 'tool_result',
  jsonb_build_object('type','tool_result','tool_use_id','toolu_cancelled','name','execute_command',
    'server_id','b4d4e17b-55eb-56ce-9bc5-cbc03fd597fd',
    'content','Cancelled by user before completion.','is_error',true));

-- C11: LARGE / TRUNCATED result (points at get_tool_result)
SELECT pg_temp.msg('30000000-0000-0000-0000-000000000019', 'assistant', 25);
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000019', 0, 'tool_use',
  jsonb_build_object('type','tool_use','id','toolu_big','name','execute_command',
    'server_id','b4d4e17b-55eb-56ce-9bc5-cbc03fd597fd',
    'input', jsonb_build_object('command','seq 1 100000')));
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000019', 1, 'tool_result',
  jsonb_build_object('type','tool_result','tool_use_id','toolu_big','name','execute_command',
    'server_id','b4d4e17b-55eb-56ce-9bc5-cbc03fd597fd',
    'content', $tr$1
2
3
... [output truncated — 99,940 lines omitted] ...
Full result available via get_tool_result(tool_use_id="toolu_big").$tr$,
    'is_error', false,
    'structured_content', jsonb_build_object('truncated', true, 'total_lines', 100000)));
-- C12: MULTIPLE resource_links of different mime types in ONE tool_result —
-- exercises every inline file viewer (image / PDF / CSV / multi-sheet XLSX) at once.
SELECT pg_temp.msg('30000000-0000-0000-0000-000000000043', 'user', 25.1);
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000043', 0, 'text',
  jsonb_build_object('type','text','text', $u$Export the analysis as several files.$u$));

SELECT pg_temp.msg('30000000-0000-0000-0000-000000000044', 'assistant', 25.2);
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000044', 0, 'tool_use',
  jsonb_build_object('type','tool_use','id','toolu_multi','name','execute_command',
    'server_id','b4d4e17b-55eb-56ce-9bc5-cbc03fd597fd',
    'input', jsonb_build_object('command','python export_all.py')));
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000044', 1, 'tool_result',
  jsonb_build_object('type','tool_result','tool_use_id','toolu_multi','name','execute_command',
    'server_id','b4d4e17b-55eb-56ce-9bc5-cbc03fd597fd',
    'content','Wrote 4 artifacts: chart.png, report.pdf, data.csv, workbook.xlsx','is_error',false,
    'resource_links', jsonb_build_array(
      jsonb_build_object('uri','/api/files/f1000000-0000-0000-0000-000000000001','name','chart.png','mime_type','image/png','size',6381,'is_saved',true,'file_id','f1000000-0000-0000-0000-000000000001'),
      jsonb_build_object('uri','/api/files/f1000000-0000-0000-0000-000000000005','name','report.pdf','mime_type','application/pdf','size',631,'is_saved',true,'file_id','f1000000-0000-0000-0000-000000000005'),
      jsonb_build_object('uri','/api/files/f1000000-0000-0000-0000-000000000004','name','data.csv','mime_type','text/csv','size',133,'is_saved',true,'file_id','f1000000-0000-0000-0000-000000000004'),
      jsonb_build_object('uri','/api/files/f1000000-0000-0000-0000-000000000003','name','workbook.xlsx','mime_type','application/vnd.openxmlformats-officedocument.spreadsheetml.sheet','size',6641,'is_saved',true,'file_id','f1000000-0000-0000-0000-000000000003'))));

-- C13: EXTERNAL (non-built-in) MCP server tool call
SELECT pg_temp.msg('30000000-0000-0000-0000-000000000045', 'user', 25.3);
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000045', 0, 'text',
  jsonb_build_object('type','text','text', $u$What is the weather in Tokyo?$u$));

SELECT pg_temp.msg('30000000-0000-0000-0000-000000000046', 'assistant', 25.4);
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000046', 0, 'tool_use',
  jsonb_build_object('type','tool_use','id','toolu_ext','name','get_weather',
    'server_id','e0000000-0000-0000-0000-0000000000e1',
    'input', jsonb_build_object('city','Tokyo','units','metric')));
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000046', 1, 'tool_result',
  jsonb_build_object('type','tool_result','tool_use_id','toolu_ext','name','get_weather',
    'server_id','e0000000-0000-0000-0000-0000000000e1',
    'content','Tokyo: 24°C, partly cloudy.','is_error',false,
    'structured_content', jsonb_build_object('temp_c',24,'condition','partly cloudy')));

-- C14: IN-FLIGHT tool_use with NO tool_result yet (renders the wrench/pending state)
SELECT pg_temp.msg('30000000-0000-0000-0000-000000000047', 'assistant', 25.5);
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000047', 0, 'text',
  jsonb_build_object('type','text','text', $md$Kicking off a long-running job (no result block — pending state):$md$));
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000047', 1, 'tool_use',
  jsonb_build_object('type','tool_use','id','toolu_pending','name','execute_command',
    'server_id','b4d4e17b-55eb-56ce-9bc5-cbc03fd597fd',
    'input', jsonb_build_object('command','python train.py')));

-- C15: EMPTY input {} + TIMEOUT-status result
SELECT pg_temp.msg('30000000-0000-0000-0000-000000000048', 'assistant', 25.6);
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000048', 0, 'tool_use',
  jsonb_build_object('type','tool_use','id','toolu_timeout','name','list_workspace',
    'server_id','b4d4e17b-55eb-56ce-9bc5-cbc03fd597fd',
    'input', jsonb_build_object()));
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000048', 1, 'tool_result',
  jsonb_build_object('type','tool_result','tool_use_id','toolu_timeout','name','list_workspace',
    'server_id','b4d4e17b-55eb-56ce-9bc5-cbc03fd597fd',
    'content','Tool call timed out after 30s.','is_error',true));
-- -- add more tool-call turns here (remember to add an mcp_tool_calls row) --

-- ###########################################################################
-- SECTION D — FILES
-- ###########################################################################

-- D1: user attaches an IMAGE (png) as an `image` block + jpg, asks about them
SELECT pg_temp.msg('30000000-0000-0000-0000-00000000001a', 'user', 26);
SELECT pg_temp.blk('30000000-0000-0000-0000-00000000001a', 0, 'image',
  jsonb_build_object('type','image',
    'source', jsonb_build_object('type','file','file_id','f1000000-0000-0000-0000-000000000001'),
    'alt_text','Bar chart PNG'));
SELECT pg_temp.blk('30000000-0000-0000-0000-00000000001a', 1, 'image',
  jsonb_build_object('type','image',
    'source', jsonb_build_object('type','file','file_id','f1000000-0000-0000-0000-000000000002'),
    'alt_text','Gradient JPG'));
SELECT pg_temp.blk('30000000-0000-0000-0000-00000000001a', 2, 'text',
  jsonb_build_object('type','text','text', $u$What do these two images show?$u$));

SELECT pg_temp.msg('30000000-0000-0000-0000-00000000001b', 'assistant', 27);
SELECT pg_temp.blk('30000000-0000-0000-0000-00000000001b', 0, 'text',
  jsonb_build_object('type','text','text', $md$The first is a quarterly bar chart (PNG); the second is a color gradient (JPG).$md$));

-- D2: user attaches PDF, CSV, XLSX, code, md, large text as file_attachment blocks
SELECT pg_temp.msg('30000000-0000-0000-0000-00000000001c', 'user', 28);
SELECT pg_temp.blk('30000000-0000-0000-0000-00000000001c', 0, 'file_attachment',
  jsonb_build_object('type','file_attachment','file_id','f1000000-0000-0000-0000-000000000005','filename','report.pdf','mime_type','application/pdf','file_size',631));
SELECT pg_temp.blk('30000000-0000-0000-0000-00000000001c', 1, 'file_attachment',
  jsonb_build_object('type','file_attachment','file_id','f1000000-0000-0000-0000-000000000004','filename','data.csv','mime_type','text/csv','file_size',133));
SELECT pg_temp.blk('30000000-0000-0000-0000-00000000001c', 2, 'file_attachment',
  jsonb_build_object('type','file_attachment','file_id','f1000000-0000-0000-0000-000000000003','filename','workbook.xlsx','mime_type','application/vnd.openxmlformats-officedocument.spreadsheetml.sheet','file_size',6641));
SELECT pg_temp.blk('30000000-0000-0000-0000-00000000001c', 3, 'file_attachment',
  jsonb_build_object('type','file_attachment','file_id','f1000000-0000-0000-0000-000000000006','filename','script.py','mime_type','text/x-python','file_size',261));
SELECT pg_temp.blk('30000000-0000-0000-0000-00000000001c', 4, 'file_attachment',
  jsonb_build_object('type','file_attachment','file_id','f1000000-0000-0000-0000-000000000007','filename','notes.md','mime_type','text/markdown','file_size',292));
SELECT pg_temp.blk('30000000-0000-0000-0000-00000000001c', 5, 'file_attachment',
  jsonb_build_object('type','file_attachment','file_id','f1000000-0000-0000-0000-000000000008','filename','large.txt','mime_type','text/plain','file_size',86400));
SELECT pg_temp.blk('30000000-0000-0000-0000-00000000001c', 6, 'text',
  jsonb_build_object('type','text','text', $u$Here are several files — the xlsx has multiple sheets.$u$));

SELECT pg_temp.msg('30000000-0000-0000-0000-00000000001d', 'assistant', 29);
SELECT pg_temp.blk('30000000-0000-0000-0000-00000000001d', 0, 'text',
  jsonb_build_object('type','text','text', $md$Received: a PDF, a CSV, a 3-sheet workbook (**Summary / Regions / Raw**), a Python file, a markdown file, and a large text file.$md$));

-- D3: MIXED assistant message — text + tool_use + tool_result + a resource_link file
SELECT pg_temp.msg('30000000-0000-0000-0000-00000000001e', 'user', 30);
SELECT pg_temp.blk('30000000-0000-0000-0000-00000000001e', 0, 'text',
  jsonb_build_object('type','text','text', $u$Summarize data.csv and regenerate the chart in one step.$u$));

SELECT pg_temp.msg('30000000-0000-0000-0000-00000000001f', 'assistant', 31);
SELECT pg_temp.blk('30000000-0000-0000-0000-00000000001f', 0, 'text',
  jsonb_build_object('type','text','text', $md$Reading the CSV and re-plotting:$md$));
SELECT pg_temp.blk('30000000-0000-0000-0000-00000000001f', 1, 'tool_use',
  jsonb_build_object('type','tool_use','id','toolu_mixed','name','execute_command',
    'server_id','b4d4e17b-55eb-56ce-9bc5-cbc03fd597fd',
    'input', jsonb_build_object('command','python summarize.py data.csv')));
SELECT pg_temp.blk('30000000-0000-0000-0000-00000000001f', 2, 'tool_result',
  jsonb_build_object('type','tool_result','tool_use_id','toolu_mixed','name','execute_command',
    'server_id','b4d4e17b-55eb-56ce-9bc5-cbc03fd597fd',
    'content','5 genes; highest expression MYC (9.88). Chart saved.','is_error',false,
    'resource_links', jsonb_build_array(jsonb_build_object(
      'uri','/api/files/f1000000-0000-0000-0000-000000000001','name','chart.png',
      'mime_type','image/png','size',6381,'is_saved',true,'file_id','f1000000-0000-0000-0000-000000000001'))));
SELECT pg_temp.blk('30000000-0000-0000-0000-00000000001f', 3, 'text',
  jsonb_build_object('type','text','text', $md$**MYC** has the highest expression (9.88). Updated chart above.$md$));

-- ###########################################################################
-- SECTION E — ELICITATION + STREAMING-STYLE LONG MESSAGE
-- ###########################################################################

-- E1: elicitation_request block (renders via ElicitationFormContent)
SELECT pg_temp.msg('30000000-0000-0000-0000-000000000020', 'assistant', 32);
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000020', 0, 'elicitation_request',
  jsonb_build_object('type','elicitation_request',
    'elicitation_id','elic-0001',
    'message','Which output format do you want for the export?',
    'server','Code Sandbox',
    'status','accepted',
    'requested_schema', jsonb_build_object(
      'type','object',
      'properties', jsonb_build_object('format', jsonb_build_object('type','string','enum', jsonb_build_array('csv','json','xlsx')))),
    'response_content', jsonb_build_object('format','xlsx')));

-- E1b: elicitation in PENDING state (renders the input form, not an outcome)
SELECT pg_temp.msg('30000000-0000-0000-0000-000000000049', 'assistant', 32.1);
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000049', 0, 'elicitation_request',
  jsonb_build_object('type','elicitation_request',
    'elicitation_id','elic-0002',
    'message','Enter a filename for the export:',
    'server','Code Sandbox',
    'status','pending',
    'requested_schema', jsonb_build_object(
      'type','object',
      'required', jsonb_build_array('filename'),
      'properties', jsonb_build_object('filename', jsonb_build_object('type','string','minLength',1)))));

-- E1c: elicitation DECLINED by the user
SELECT pg_temp.msg('30000000-0000-0000-0000-00000000004a', 'assistant', 32.2);
SELECT pg_temp.blk('30000000-0000-0000-0000-00000000004a', 0, 'elicitation_request',
  jsonb_build_object('type','elicitation_request',
    'elicitation_id','elic-0003',
    'message','Allow the tool to delete temporary files?',
    'server','Code Sandbox',
    'status','declined',
    'requested_schema', jsonb_build_object(
      'type','object',
      'properties', jsonb_build_object('confirm', jsonb_build_object('type','boolean')))));

-- E2: streaming-style long single text block
SELECT pg_temp.msg('30000000-0000-0000-0000-000000000021', 'user', 33);
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000021', 0, 'text',
  jsonb_build_object('type','text','text', $u$Give me a long, streamed explanation of vector databases.$u$));

SELECT pg_temp.msg('30000000-0000-0000-0000-000000000022', 'assistant', 34);
SELECT pg_temp.blk('30000000-0000-0000-0000-000000000022', 0, 'text',
  jsonb_build_object('type','text','text', $md$Vector databases store high-dimensional embeddings and retrieve by similarity rather than exact match. Here is the long version.

## 1. Embeddings
An embedding maps text, images, or audio into a fixed-length vector such that semantically similar inputs land near each other. Distance metrics are typically cosine, inner product, or L2.

## 2. Indexes
Brute-force search is exact but O(n). Approximate indexes trade a little recall for large speedups:

- **HNSW** — a navigable small-world graph; excellent recall/latency, higher memory.
- **IVFFlat** — clusters vectors into lists; cheaper to build, tune `lists` + `probes`.
- **PQ / OPQ** — compress vectors to cut memory, at some accuracy cost.

## 3. Tuning
For HNSW, `m` and `ef_construction` set graph quality at build time; `ef_search` trades latency for recall at query time. Measure recall@k against a ground-truth set before shipping.

## 4. Operational notes
Re-embed when you change models — vectors from different models are not comparable. Keep the original text so you can re-index. Watch index build memory on large tables, and consider partial indexes for multi-tenant data.

## 5. When NOT to use one
If your corpus is tiny, a linear scan in Postgres is simpler and exact. Vector search shines at scale and for fuzzy semantic recall, not for precise keyword lookups (use full-text search or a trigram index there).

That is the whirlwind tour — ask about any layer for a deeper dive.$md$));
-- -- add more sections/turns here --

COMMIT;

-- ===========================================================================
-- SECTION C-rows — mcp_tool_calls (the "Calls" tab / tool-call history)
--   One row per tool_use above. Owner-scoped; conversation/branch/message set.
--   status vocabulary: completed | failed | timeout | cancelled.
-- ===========================================================================
BEGIN;
INSERT INTO mcp_tool_calls
  (id, server_id, server_name, is_built_in, user_id, conversation_id, branch_id, message_id,
   tool_use_id, tool_name, arguments_json, source, status, is_error, result_json, content_kinds,
   result_bytes, started_at, finished_at, duration_ms)
VALUES
  ('7c000000-0000-0000-0000-000000000001','b4d4e17b-55eb-56ce-9bc5-cbc03fd597fd','code-sandbox',true,:'owner',
   '11111111-1111-1111-1111-111111111111','22222222-2222-2222-2222-222222222222','30000000-0000-0000-0000-00000000000a',
   'toolu_sandbox_1','execute_command','{"command":"python plot.py"}','chat','completed',false,
   '{"content":"exit_code: 0"}','{text,resource_link}',120,
   TIMESTAMPTZ '2026-07-01 12:00:10+00', TIMESTAMPTZ '2026-07-01 12:00:11+00', 812),

  ('7c000000-0000-0000-0000-000000000002','d1a783dc-631e-570b-aba6-fee5497728b2','web_search',true,:'owner',
   '11111111-1111-1111-1111-111111111111','22222222-2222-2222-2222-222222222222','30000000-0000-0000-0000-00000000000c',
   'toolu_web_1','web_search','{"query":"pgvector HNSW tuning"}','chat','completed',false,
   '{"content":"Top results"}','{text}',95, TIMESTAMPTZ '2026-07-01 12:00:12+00', TIMESTAMPTZ '2026-07-01 12:00:12+00', 430),

  ('7c000000-0000-0000-0000-000000000003','d1a783dc-631e-570b-aba6-fee5497728b2','web_search',true,:'owner',
   '11111111-1111-1111-1111-111111111111','22222222-2222-2222-2222-222222222222','30000000-0000-0000-0000-00000000000d',
   'toolu_fetch_1','fetch_url','{"url":"https://example.com/hnsw"}','chat','completed',false,
   '{"content":"# Tuning HNSW"}','{text}',180, TIMESTAMPTZ '2026-07-01 12:00:13+00', TIMESTAMPTZ '2026-07-01 12:00:13+00', 260),

  ('7c000000-0000-0000-0000-000000000004','5bf27612-ac1b-5141-985b-e2e8ac36ca2d','lit_search',true,:'owner',
   '11111111-1111-1111-1111-111111111111','22222222-2222-2222-2222-222222222222','30000000-0000-0000-0000-00000000000f',
   'toolu_lit_1','literature_search','{"query":"CRISPR base editing off-target"}','chat','completed',false,
   '{"content":"2 records"}','{text}',210, TIMESTAMPTZ '2026-07-01 12:00:15+00', TIMESTAMPTZ '2026-07-01 12:00:16+00', 990),

  ('7c000000-0000-0000-0000-000000000005','16e2eeb0-46ed-5588-af8a-e973349f99a1','memory',true,:'owner',
   '11111111-1111-1111-1111-111111111111','22222222-2222-2222-2222-222222222222','30000000-0000-0000-0000-000000000011',
   'toolu_mem_1','remember','{"content":"User prefers SI units."}','chat','completed',false,
   '{"content":"Stored memory #1."}','{text}',40, TIMESTAMPTZ '2026-07-01 12:00:17+00', TIMESTAMPTZ '2026-07-01 12:00:17+00', 55),

  ('7c000000-0000-0000-0000-000000000006','16e2eeb0-46ed-5588-af8a-e973349f99a1','memory',true,:'owner',
   '11111111-1111-1111-1111-111111111111','22222222-2222-2222-2222-222222222222','30000000-0000-0000-0000-000000000011',
   'toolu_mem_2','recall','{"query":"preferences"}','chat','completed',false,
   '{"content":"1 memory"}','{text}',60, TIMESTAMPTZ '2026-07-01 12:00:17+00', TIMESTAMPTZ '2026-07-01 12:00:17+00', 61),

  ('7c000000-0000-0000-0000-000000000007','011e52cb-2d06-5e6b-8f4c-41076519f167','citations',true,:'owner',
   '11111111-1111-1111-1111-111111111111','22222222-2222-2222-2222-222222222222','30000000-0000-0000-0000-000000000013',
   'toolu_cit_1','format_citations','{"style":"apa"}','chat','completed',false,
   '{"content":"Lee, & Gomez. (2024)."}','{text}',70, TIMESTAMPTZ '2026-07-01 12:00:19+00', TIMESTAMPTZ '2026-07-01 12:00:19+00', 88),

  ('7c000000-0000-0000-0000-000000000008','d878787e-aa48-5f16-a31f-673052083f34','control',true,:'owner',
   '11111111-1111-1111-1111-111111111111','22222222-2222-2222-2222-222222222222','30000000-0000-0000-0000-000000000014',
   'toolu_ctl_1','call_api','{"method":"GET","path":"/api/conversations"}','chat','completed',false,
   '{"content":"{ total: 1 }"}','{text}',48, TIMESTAMPTZ '2026-07-01 12:00:20+00', TIMESTAMPTZ '2026-07-01 12:00:20+00', 33),

  ('7c000000-0000-0000-0000-000000000009','62c47165-bcf4-5daf-b778-8eff985ac943','tool_result',true,:'owner',
   '11111111-1111-1111-1111-111111111111','22222222-2222-2222-2222-222222222222','30000000-0000-0000-0000-000000000015',
   'toolu_gtr_1','get_tool_result','{"tool_use_id":"toolu_web_1"}','chat','completed',false,
   '{"content":"Top results"}','{text}',60, TIMESTAMPTZ '2026-07-01 12:00:21+00', TIMESTAMPTZ '2026-07-01 12:00:21+00', 22),

  -- FAILED
  ('7c000000-0000-0000-0000-00000000000a','b4d4e17b-55eb-56ce-9bc5-cbc03fd597fd','code-sandbox',true,:'owner',
   '11111111-1111-1111-1111-111111111111','22222222-2222-2222-2222-222222222222','30000000-0000-0000-0000-000000000017',
   'toolu_sandbox_err','execute_command','{"command":"cat /nope"}','chat','failed',true,
   '{"content":"No such file"}','{text}',60, TIMESTAMPTZ '2026-07-01 12:00:23+00', TIMESTAMPTZ '2026-07-01 12:00:23+00', 40),

  -- CANCELLED
  ('7c000000-0000-0000-0000-00000000000b','b4d4e17b-55eb-56ce-9bc5-cbc03fd597fd','code-sandbox',true,:'owner',
   '11111111-1111-1111-1111-111111111111','22222222-2222-2222-2222-222222222222','30000000-0000-0000-0000-000000000018',
   'toolu_cancelled','execute_command','{"command":"sleep 999"}','chat','cancelled',true,
   '{"content":"Cancelled"}','{text}',30, TIMESTAMPTZ '2026-07-01 12:00:24+00', TIMESTAMPTZ '2026-07-01 12:00:25+00', 1000),

  -- LARGE / truncated (completed)
  ('7c000000-0000-0000-0000-00000000000c','b4d4e17b-55eb-56ce-9bc5-cbc03fd597fd','code-sandbox',true,:'owner',
   '11111111-1111-1111-1111-111111111111','22222222-2222-2222-2222-222222222222','30000000-0000-0000-0000-000000000019',
   'toolu_big','execute_command','{"command":"seq 1 100000"}','chat','completed',false,
   '{"content":"truncated"}','{text}',1048576, TIMESTAMPTZ '2026-07-01 12:00:25+00', TIMESTAMPTZ '2026-07-01 12:00:27+00', 1500),

  -- MIXED message tool call
  ('7c000000-0000-0000-0000-00000000000d','b4d4e17b-55eb-56ce-9bc5-cbc03fd597fd','code-sandbox',true,:'owner',
   '11111111-1111-1111-1111-111111111111','22222222-2222-2222-2222-222222222222','30000000-0000-0000-0000-00000000001f',
   'toolu_mixed','execute_command','{"command":"python summarize.py"}','chat','completed',false,
   '{"content":"5 genes"}','{text,resource_link}',140, TIMESTAMPTZ '2026-07-01 12:00:31+00', TIMESTAMPTZ '2026-07-01 12:00:31+00', 300),

  -- MULTI-file resource_links
  ('7c000000-0000-0000-0000-00000000000e','b4d4e17b-55eb-56ce-9bc5-cbc03fd597fd','code-sandbox',true,:'owner',
   '11111111-1111-1111-1111-111111111111','22222222-2222-2222-2222-222222222222','30000000-0000-0000-0000-000000000044',
   'toolu_multi','execute_command','{"command":"python export_all.py"}','chat','completed',false,
   '{"content":"4 artifacts"}','{text,resource_link}',260, TIMESTAMPTZ '2026-07-01 12:00:26+00', TIMESTAMPTZ '2026-07-01 12:00:27+00', 640),

  -- EXTERNAL (non-built-in) server; triggered "always" (auto-approved) path
  ('7c000000-0000-0000-0000-00000000000f','e0000000-0000-0000-0000-0000000000e1','weather-api',false,:'owner',
   '11111111-1111-1111-1111-111111111111','22222222-2222-2222-2222-222222222222','30000000-0000-0000-0000-000000000046',
   'toolu_ext','get_weather','{"city":"Tokyo"}','always','completed',false,
   '{"content":"24C"}','{text}',60, TIMESTAMPTZ '2026-07-01 12:00:28+00', TIMESTAMPTZ '2026-07-01 12:00:28+00', 210),

  -- IN-FLIGHT tool_use has no result row (nothing recorded until it returns) —
  -- intentionally omitted here to mirror reality.

  -- TIMEOUT status
  ('7c000000-0000-0000-0000-000000000010','b4d4e17b-55eb-56ce-9bc5-cbc03fd597fd','code-sandbox',true,:'owner',
   '11111111-1111-1111-1111-111111111111','22222222-2222-2222-2222-222222222222','30000000-0000-0000-0000-000000000048',
   'toolu_timeout','list_workspace','{}','chat','timeout',true,
   '{"content":"timed out"}','{text}',20, TIMESTAMPTZ '2026-07-01 12:00:29+00', TIMESTAMPTZ '2026-07-01 12:00:59+00', 30000),

  -- STANDALONE calls (no conversation) exercising the remaining `source` values
  -- so the Calls-tab source filter has every variant (chat/always above; rest/sampling/approval here).
  ('7c000000-0000-0000-0000-000000000011','16e2eeb0-46ed-5588-af8a-e973349f99a1','memory',true,:'owner',
   NULL,NULL,NULL,NULL,'recall','{"query":"units"}','rest','completed',false,
   '{"content":"1 memory"}','{text}',60, TIMESTAMPTZ '2026-07-01 12:01:00+00', TIMESTAMPTZ '2026-07-01 12:01:00+00', 44),
  ('7c000000-0000-0000-0000-000000000012','d1a783dc-631e-570b-aba6-fee5497728b2','web_search',true,:'owner',
   '11111111-1111-1111-1111-111111111111','22222222-2222-2222-2222-222222222222',NULL,
   'toolu_samp_1','web_search','{"query":"embeddings"}','sampling','completed',false,
   '{"content":"results"}','{text}',90, TIMESTAMPTZ '2026-07-01 12:01:01+00', TIMESTAMPTZ '2026-07-01 12:01:01+00', 130),
  ('7c000000-0000-0000-0000-000000000013','b4d4e17b-55eb-56ce-9bc5-cbc03fd597fd','code-sandbox',true,:'owner',
   '11111111-1111-1111-1111-111111111111','22222222-2222-2222-2222-222222222222',NULL,
   'toolu_appr_1','execute_command','{"command":"whoami"}','approval','completed',false,
   '{"content":"ok"}','{text}',30, TIMESTAMPTZ '2026-07-01 12:01:02+00', TIMESTAMPTZ '2026-07-01 12:01:02+00', 25)
ON CONFLICT (id) DO NOTHING;
-- -- add more mcp_tool_calls rows here --
COMMIT;

-- ===========================================================================
-- 4. SCENARIO CONVERSATIONS (stateful surfaces shown in ISOLATION)
-- ===========================================================================
-- The conversation above is the exhaustive scroll-through reference. These
-- extra, small conversations each isolate ONE state so it's obvious in the
-- sidebar — especially the two seedable "waiting" states:
--
--   * Tool call AWAITING APPROVAL — a `tool_use_approvals` row with
--     status='pending' + a tool_use block with no result. On conversation open
--     the MCP chat-extension fetches GET /branches/{id}/pending-approvals and
--     re-hydrates the approval panel (McpToolCallUI → ToolCallPendingApprovalContent).
--   * Elicitation WAITING — an `elicitation_request` content block, status='pending'.
--
-- NOT seedable: a "Running…" (started) tool call — that status lives only in the
-- live SSE stream, never persisted; on reload it shows as a result-less tool_use.
--
-- All share the "UI Showcase" project + the same built-in server rows.
-- ===========================================================================
BEGIN;

-- conv(): conversation + branch + active-branch link + project membership.
-- owner is passed in (a temp-function body can't see the :owner psql var).
CREATE OR REPLACE FUNCTION pg_temp.conv(cid uuid, bid uuid, ctitle text, owner uuid) RETURNS void AS $fn$
BEGIN
  INSERT INTO conversations (id, user_id, title, created_at)
    VALUES (cid, owner, ctitle, TIMESTAMPTZ '2026-07-02 09:00:00+00')
    ON CONFLICT (id) DO NOTHING;
  INSERT INTO branches (id, conversation_id, created_at)
    VALUES (bid, cid, TIMESTAMPTZ '2026-07-02 09:00:00+00')
    ON CONFLICT (id) DO NOTHING;
  UPDATE conversations SET active_branch_id = bid WHERE id = cid;
  INSERT INTO project_conversations (conversation_id, project_id)
    VALUES (cid, '90000000-0000-0000-0000-000000000001')
    ON CONFLICT (conversation_id) DO NOTHING;
END;
$fn$ LANGUAGE plpgsql;

-- cmsg(): like pg_temp.msg() but targets an arbitrary branch.
CREATE OR REPLACE FUNCTION pg_temp.cmsg(mid uuid, bid uuid, mrole text, n numeric) RETURNS void AS $fn$
BEGIN
  INSERT INTO messages (id, role, originated_from_id, edit_count, created_at)
    VALUES (mid, mrole, mid, 0, TIMESTAMPTZ '2026-07-02 09:00:00+00' + (n || ' seconds')::interval)
    ON CONFLICT (id) DO NOTHING;
  INSERT INTO branch_messages (branch_id, message_id, is_clone, created_at)
    VALUES (bid, mid, false, TIMESTAMPTZ '2026-07-02 09:00:00+00' + (n || ' seconds')::interval)
    ON CONFLICT (branch_id, message_id) DO NOTHING;
END;
$fn$ LANGUAGE plpgsql;

-- ---- Scenario 1: TOOL CALL — AWAITING APPROVAL (the pending approval panel) --
SELECT pg_temp.conv('10000000-0000-0000-0000-0000000000c1','20000000-0000-0000-0000-0000000000c1',
                    'Scenario · Tool call — awaiting approval', :'owner');
SELECT pg_temp.cmsg('3c100000-0000-0000-0000-000000000001','20000000-0000-0000-0000-0000000000c1','user',1);
SELECT pg_temp.blk('3c100000-0000-0000-0000-000000000001',0,'text',
  jsonb_build_object('type','text','text', $u$Delete the temp build directory.$u$));
SELECT pg_temp.cmsg('3c100000-0000-0000-0000-000000000002','20000000-0000-0000-0000-0000000000c1','assistant',2);
SELECT pg_temp.blk('3c100000-0000-0000-0000-000000000002',0,'text',
  jsonb_build_object('type','text','text', $md$This needs your approval before running:$md$));
-- tool_use block WITH NO tool_result → paired with the pending approval row below.
SELECT pg_temp.blk('3c100000-0000-0000-0000-000000000002',1,'tool_use',
  jsonb_build_object('type','tool_use','id','toolu_await_approval','name','execute_command',
    'server_id','b4d4e17b-55eb-56ce-9bc5-cbc03fd597fd',
    'input', jsonb_build_object('command','rm -rf ./build/tmp')));

-- The row the frontend re-hydrates into the approval panel on conversation open.
INSERT INTO tool_use_approvals
  (id, conversation_id, branch_id, message_id, user_id, tool_use_id, tool_name, tool_input, server_id, server_name, status)
VALUES
  ('a9900000-0000-0000-0000-000000000001',
   '10000000-0000-0000-0000-0000000000c1','20000000-0000-0000-0000-0000000000c1','3c100000-0000-0000-0000-000000000002',
   :'owner','toolu_await_approval','execute_command',
   jsonb_build_object('command','rm -rf ./build/tmp'),
   'b4d4e17b-55eb-56ce-9bc5-cbc03fd597fd','Code Sandbox','pending')
ON CONFLICT (message_id, tool_use_id) DO NOTHING;

-- ---- Scenario 2: TOOL CALL — COMPLETED (one clean done call, in isolation) --
SELECT pg_temp.conv('10000000-0000-0000-0000-0000000000c2','20000000-0000-0000-0000-0000000000c2',
                    'Scenario · Tool call — completed', :'owner');
SELECT pg_temp.cmsg('3c200000-0000-0000-0000-000000000001','20000000-0000-0000-0000-0000000000c2','user',1);
SELECT pg_temp.blk('3c200000-0000-0000-0000-000000000001',0,'text',
  jsonb_build_object('type','text','text', $u$Search for "vector index recall".$u$));
SELECT pg_temp.cmsg('3c200000-0000-0000-0000-000000000002','20000000-0000-0000-0000-0000000000c2','assistant',2);
SELECT pg_temp.blk('3c200000-0000-0000-0000-000000000002',0,'tool_use',
  jsonb_build_object('type','tool_use','id','toolu_done_1','name','web_search',
    'server_id','d1a783dc-631e-570b-aba6-fee5497728b2',
    'input', jsonb_build_object('query','vector index recall','max_results',2)));
SELECT pg_temp.blk('3c200000-0000-0000-0000-000000000002',1,'tool_result',
  jsonb_build_object('type','tool_result','tool_use_id','toolu_done_1','name','web_search',
    'server_id','d1a783dc-631e-570b-aba6-fee5497728b2',
    'content','2 results found.','is_error',false,
    'structured_content', jsonb_build_object('provider','searxng','results', jsonb_build_array(
      jsonb_build_object('title','Recall@k explained','url','https://example.com/recall','snippet','recall measures...')))));
SELECT pg_temp.blk('3c200000-0000-0000-0000-000000000002',2,'text',
  jsonb_build_object('type','text','text', $md$Found 2 results — recall@k measures retrieved relevance.$md$));

INSERT INTO mcp_tool_calls
  (id, server_id, server_name, is_built_in, user_id, conversation_id, branch_id, message_id,
   tool_use_id, tool_name, arguments_json, source, status, is_error, result_json, content_kinds, result_bytes,
   started_at, finished_at, duration_ms)
VALUES
  ('7c100000-0000-0000-0000-000000000001','d1a783dc-631e-570b-aba6-fee5497728b2','web_search',true,:'owner',
   '10000000-0000-0000-0000-0000000000c2','20000000-0000-0000-0000-0000000000c2','3c200000-0000-0000-0000-000000000002',
   'toolu_done_1','web_search','{"query":"vector index recall"}','chat','completed',false,
   '{"content":"2 results"}','{text}',80, TIMESTAMPTZ '2026-07-02 09:00:02+00', TIMESTAMPTZ '2026-07-02 09:00:02+00', 250)
ON CONFLICT (id) DO NOTHING;

-- ---- Scenario 3: ELICITATION — WAITING for input (pending form) --------------
SELECT pg_temp.conv('10000000-0000-0000-0000-0000000000c3','20000000-0000-0000-0000-0000000000c3',
                    'Scenario · Elicitation — waiting for input', :'owner');
SELECT pg_temp.cmsg('3c300000-0000-0000-0000-000000000001','20000000-0000-0000-0000-0000000000c3','user',1);
SELECT pg_temp.blk('3c300000-0000-0000-0000-000000000001',0,'text',
  jsonb_build_object('type','text','text', $u$Export the results.$u$));
SELECT pg_temp.cmsg('3c300000-0000-0000-0000-000000000002','20000000-0000-0000-0000-0000000000c3','assistant',2);
SELECT pg_temp.blk('3c300000-0000-0000-0000-000000000002',0,'elicitation_request',
  jsonb_build_object('type','elicitation_request',
    'elicitation_id','elic-scn-01',
    'message','Choose an export format and filename:',
    'server','Code Sandbox',
    'status','pending',
    'requested_schema', jsonb_build_object('type','object',
      'required', jsonb_build_array('format','filename'),
      'properties', jsonb_build_object(
        'format', jsonb_build_object('type','string','enum', jsonb_build_array('csv','json','xlsx')),
        'filename', jsonb_build_object('type','string','minLength',1)))));

-- ---- Scenario 4: ELICITATION — RESOLVED (accepted + declined side by side) ---
SELECT pg_temp.conv('10000000-0000-0000-0000-0000000000c4','20000000-0000-0000-0000-0000000000c4',
                    'Scenario · Elicitation — resolved', :'owner');
SELECT pg_temp.cmsg('3c400000-0000-0000-0000-000000000001','20000000-0000-0000-0000-0000000000c4','assistant',1);
SELECT pg_temp.blk('3c400000-0000-0000-0000-000000000001',0,'elicitation_request',
  jsonb_build_object('type','elicitation_request','elicitation_id','elic-scn-02',
    'message','Which output format?','server','Code Sandbox','status','accepted',
    'requested_schema', jsonb_build_object('type','object','properties', jsonb_build_object(
      'format', jsonb_build_object('type','string','enum', jsonb_build_array('csv','xlsx')))),
    'response_content', jsonb_build_object('format','xlsx')));
SELECT pg_temp.cmsg('3c400000-0000-0000-0000-000000000002','20000000-0000-0000-0000-0000000000c4','assistant',2);
SELECT pg_temp.blk('3c400000-0000-0000-0000-000000000002',0,'elicitation_request',
  jsonb_build_object('type','elicitation_request','elicitation_id','elic-scn-03',
    'message','Allow deleting temporary files?','server','Code Sandbox','status','declined',
    'requested_schema', jsonb_build_object('type','object','properties', jsonb_build_object(
      'confirm', jsonb_build_object('type','boolean')))));
-- -- add more scenario conversations here --

COMMIT;

-- Done. Load bytes with load.sh, then open the conversations in the UI:
--   * "Rendering Showcase — every block type"  (the exhaustive reference)
--   * "Scenario · …"                            (one state each, incl. the two
--                                                seedable "waiting" states)

