-- 全局资产改为「始终全集」语境(2026-07 实测定稿):
-- 人物档案/主线结构 prompt 更新为全集+集边界表;节拍表从 C 文本聚合改为全集视频直出;
-- 钩子链并入付费卡点推断。删除旧 builtin 行,由 seed_asset_specs 按新定义重插
-- (用户自建资产不受影响;dna_tasks 级联清理,重跑即按新定义建单)。
DELETE FROM asset_specs
WHERE id IN ('a-characters', 'a-storyline', 'c-beatsheet', 'c-hooks') AND builtin = 1;
