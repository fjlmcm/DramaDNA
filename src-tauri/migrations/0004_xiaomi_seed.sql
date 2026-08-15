-- 给已 seed 的老用户库新增小米 MiMo 预设供应商与模型。
-- 仅当 default_seeded 标志已存在(库被 seed 过)时插入 —— 新装用户由 db.rs
-- seed_defaults 直接 INSERT 包含小米的完整 4 家,无需走这个 migration。
-- 双重 NOT EXISTS 防止用户手动加过同 id 时被覆盖。
--
-- 选值依据(2026-05 实测 + 文档):
--   小米 mimo-v2.5 base64 上限文档 50MB → 原始 ≤ 37MB,设 35 MiB(36700160)
--   鉴权用自定义 api-key header(非 Bearer),由 provider.rs 适配

INSERT INTO providers (id, name, kind, base_url, api_key, extra_config, created_at, updated_at)
SELECT
  'seed-xiaomi', '小米 MiMo', 'xiaomi', 'https://api.xiaomimimo.com/v1',
  '', '{}',
  strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
  strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE EXISTS (SELECT 1 FROM settings WHERE key = 'default_seeded')
  AND NOT EXISTS (SELECT 1 FROM providers WHERE id = 'seed-xiaomi');

INSERT INTO models (id, provider_id, model_id, display_name, video_input_method, constraints, params, enabled, created_at, updated_at)
SELECT
  'seed-xiaomi-m', 'seed-xiaomi', 'mimo-v2.5', '小米 MiMo 2.5', 'base64',
  '{"maxBytes":36700160}', '{}', 1,
  strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
  strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE EXISTS (SELECT 1 FROM settings WHERE key = 'default_seeded')
  AND NOT EXISTS (SELECT 1 FROM models WHERE id = 'seed-xiaomi-m');
