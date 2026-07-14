-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- mcp seed data.

INSERT INTO public.mcp_servers VALUES ('865f06fa-c4e5-4eb3-9801-5804f67062c2', NULL, 'fetch', 'Web Fetch', 'Fetch content from web URLs', true, true, 'stdio', 'uvx', '["mcp-server-fetch"]', '{}', NULL, '{}', 30, '2026-07-14 15:09:55.65088+00', '2026-07-14 15:09:55.65088+00', false, 'auto', NULL, true, false, '{}', '{}', '{}', '{}', NULL, 'untested', NULL, 'full');

INSERT INTO public.mcp_user_policy VALUES (1, '{http,stdio}', 'full', '2026-07-14 15:09:58.735031+00', NULL, 90);

INSERT INTO public.user_group_mcp_servers VALUES ('79668b78-16ff-4d3f-80e6-afdbb3236dc9', '865f06fa-c4e5-4eb3-9801-5804f67062c2', '2026-07-14 15:09:55.651712+00');
