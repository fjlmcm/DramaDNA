-- 台词标注改为视频任务(2026-07 实测:单集视频+人物档案+台词原文对照,
-- 画面证据(口型/正反打)纠正纯文本反推的归属错误并消除存疑标注)。
-- 删除旧 builtin 行,由 seed_asset_specs 按新定义重插。
DELETE FROM asset_specs WHERE id = 'c-annotated' AND builtin = 1;
