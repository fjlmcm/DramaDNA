-- 证据链升级:拆解卡注入同集台词原文(金句逐字);金句库以台词原文逐字校对。
-- 删除旧 builtin 行,由 seed_asset_specs 按新定义重插。
DELETE FROM asset_specs WHERE id IN ('b-breakdown', 'c-quotes') AND builtin = 1;
