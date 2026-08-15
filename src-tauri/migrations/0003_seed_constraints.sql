-- 给三家预设模型设上各自 API 上限反推的 max_bytes。
-- 用 json_set 只改 maxBytes 字段 —— 保留用户已设的 maxWidth / maxHeight / maxFps 等偏好。
--
-- 选值依据(2026-05 实测):
--   豆包请求体 64MB → 原始 ≤ 45MB,设 43 MiB(45088768)
--   通义 data-uri 20MB → 原始 ≤ 15MB,设 14 MiB(14680064)
--   云雾(中转)base64 ~13MB ✓ / 15MB ✗ → 原始 ≤ 10MB,设 8 MiB(8388608)
--     (云雾是第三方中转站,实际上限远低于 Gemini 官方,以中转站为准)

UPDATE models SET constraints = json_set(
  CASE WHEN constraints = '' OR constraints IS NULL THEN '{}' ELSE constraints END,
  '$.maxBytes', 45088768
) WHERE id = 'seed-volc-m';

UPDATE models SET constraints = json_set(
  CASE WHEN constraints = '' OR constraints IS NULL THEN '{}' ELSE constraints END,
  '$.maxBytes', 14680064
) WHERE id = 'seed-ali-m';

UPDATE models SET constraints = json_set(
  CASE WHEN constraints = '' OR constraints IS NULL THEN '{}' ELSE constraints END,
  '$.maxBytes', 8388608
) WHERE id = 'seed-yun-m';
