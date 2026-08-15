// 内置资产规格与 prompt —— DramaDNA 的领域核心。
//
// 原则:每次模型调用只产出一类资产。占位符由 pipeline 在执行时替换:
//   {drama_name} {episode_count} {ep_no} {ep_title} {ep_range} {segment_no}
//   {segment_count} {ep_titles} {user_input}
// 依赖资产(inputs)由 pipeline 以"## 参考资料"小节附加在 prompt 之后。

/// 内置资产规格 —— 由 db::seed_asset_specs 幂等写入 asset_specs 表。
pub struct BuiltinSpec {
    pub id: &'static str,
    pub stage: &'static str, // global(A) | episode(B) | synth(C) | adapt(D)
    pub sort_no: i64,
    pub name: &'static str,
    pub scope: &'static str, // per_segment | per_episode | per_drama
    pub prompt: &'static str,
    pub merge_prompt: Option<&'static str>,
    pub inputs: &'static str, // json: ["spec_id"] 同集/最终稿,"spec_id:all" 全集聚合
    pub output_template: &'static str,
    pub needs_video: bool,
    pub user_input: bool,
    pub params: &'static str, // json: 请求参数覆盖
}

/// 所有任务共用的上下文头 —— pipeline 拼在 prompt 最前。
pub const CONTEXT_HEADER: &str = "\
你是资深短剧编剧与拆解顾问。当前拆解对象:竖屏短剧《{drama_name}》,共 {episode_count} 集。
要求:直接输出 markdown 正文(不要用代码块包裹,不要输出任何解释性开场白或结束语);\
只依据给到的视频与参考资料,不编造;不确定的信息明确标注「?」。\n\n";

/// per_segment 资产的默认分段合并 prompt。
pub const DEFAULT_MERGE_PROMPT: &str = "\
以下是《{drama_name}》按分段(共 {segment_count} 段)分别提取的「{asset_name}」草稿。\
请合并为一份完整定稿:去重合并同一对象的信息;前后段信息冲突时以靠后段为准\
(剧情推进后信息更完整,如身份揭晓、关系变化要体现最终认知,并保留「早期伪装→后期揭示」的演进说明);\
保持原有 markdown 结构。";

pub const BUILTIN_SPECS: &[BuiltinSpec] = &[
    // ────────────────────── Stage A:全局资产(分段视频) ──────────────────────
    BuiltinSpec {
        id: "a-characters",
        stage: "global",
        sort_no: 1,
        name: "人物档案",
        scope: "per_segment",
        prompt: "\
本视频是《{drama_name}》全剧完整拼接(共 {episode_count} 集)。各集时间范围:

{ep_timeline}

只做一件事:为全剧出现的每个人物建档。

输出格式(每个人物一节,主角在前、按戏份排序):

## 人物名
- **称呼来源**:名字依据(字幕称呼/自我介绍/他人称呼);无名者以「角色描述」为标题(如「男中介」)并标「?」
- **外貌特征**:发型、衣着、年龄段、辨识特征(用于跨集认人,尽量具体)
- **身份职业**:表面身份;若有隐藏身份写「表面 X / 实际 Y(第N集揭示,对照上方时间表)」
- **动机目标**:这个人物想要什么、怕什么
- **人物弧光**:从开场到结局的转变
- **人物关系**:与其他人物的关系(格式:与XX——关系及性质)

要求:不遗漏有台词的人物;纯背景路人忽略;特别注意覆盖全剧中后段的身份揭示与结局走向。",
        merge_prompt: None,
        inputs: "[]",
        output_template: "01-人物档案.md",
        needs_video: true,
        user_input: false,
        params: r#"{}"#,
    },
    BuiltinSpec {
        id: "a-storyline",
        stage: "global",
        sort_no: 2,
        name: "主线结构",
        scope: "per_segment",
        prompt: "\
本视频是《{drama_name}》全剧完整拼接(共 {episode_count} 集)。各集时间范围:

{ep_timeline}

只做一件事:梳理全剧主线结构。

输出格式:

## 全剧梗概
(300 字内,完整讲一遍故事)

## 主线脉络
(按阶段/幕划分,每阶段:集数范围 | 阶段任务 | 核心冲突态势)

## 关键节点
(定位以下节点在第几集,各用一句话说明:激励事件/第一次打脸/第一次身份线索泄露/\
中点大转折/至暗时刻/最终对决/结局收束;没有的节点写「无」)

## 支线清单
(每条支线:涉及人物、起止集数、与主线的关系)

## 伏笔-揭示对照表
(表格:伏笔 | 埋设集数 | 揭晓集数 | 作用)",
        merge_prompt: None,
        inputs: "[]",
        output_template: "02-主线结构.md",
        needs_video: true,
        user_input: false,
        params: r#"{}"#,
    },
    BuiltinSpec {
        id: "a-visuals",
        stage: "global",
        sort_no: 4,
        name: "视觉设定",
        scope: "per_segment",
        prompt: "\
本视频是《{drama_name}》全剧完整拼接(共 {episode_count} 集)。各集时间范围:

{ep_timeline}

只做一件事:为下游 AI 视频生成(文生图/图生视频)建立全剧视觉设定档案。\
人物一律使用参考资料人物档案中的名字。

输出格式:

## 人物视觉卡
(每个主要人物一节,按戏份排序)

### 人物名
- **定妆描述块**:一段可直接粘贴进文生图提示词的完整外观描述——性别、年龄段、\
脸型五官、发型发色、体型身姿、气质关键词(如「清冷」「痞帅」);只写看得见的,不带剧情评价
- **服装分期**:按剧情阶段列出主要造型(集数范围 | 服装描述 | 出现场合)
- **标志性细节**:跨镜头保持一致的辨识物(饰品、疤痕、眼镜、发饰等);没有写「无」

## 场景视觉库
(每个高频场景一节)

### 场景名
- **空间与陈设**:空间结构、主要陈设、材质
- **光线与色调**:主光源、时段、色温倾向
- **气质**:一句话概括(如「老钱风会客厅」「破败城中村出租屋」)
- **出现集数**:主要出现在哪些集

## 关键道具
(承担剧情功能的道具,表格:道具名 | 视觉描述 | 剧情功能 | 首次出现集数)",
        merge_prompt: None,
        inputs: r#"["a-characters"]"#,
        output_template: "08-视觉设定.md",
        needs_video: true,
        user_input: false,
        params: r#"{}"#,
    },
    BuiltinSpec {
        id: "a-cinematography",
        stage: "global",
        sort_no: 5,
        name: "视听语言",
        scope: "per_segment",
        prompt: "\
本视频是《{drama_name}》全剧完整拼接(共 {episode_count} 集)。各集时间范围:

{ep_timeline}

只做一件事:提炼全剧的视听语言档案——下游用它生成同风格的新视频,结论要可执行、可复制。

输出格式:

## 镜头语言
- **景别习惯**:特写/近景/中景/全景的大致占比与各自用途(给出本剧的实际观察,不套通则)
- **运镜习惯**:常用运镜(推/拉/摇/移/手持/固定)及各自出现的情绪场合
- **对话调度**:正反打节奏、单人镜头与双人同框的使用习惯
- **高光时刻处理**:打脸/揭示/暴击瞬间的镜头套路(慢动作、急推、闪回插切等)

## 剪辑节奏
- **镜头时长**:平均镜头时长估计;叙事段与冲突段的剪辑率差异
- **转场方式**:场与场、集与集之间的转场习惯
- **开场与结尾**:每集开场方式(冷开场/延续上集)与结尾定格的处理

## 光线与调色
- **整体影调**:明暗倾向、色温基调、饱和度
- **场景差异**:不同类型场景(豪宅/职场/街头/夜戏)的打光与调色差异

## 竖屏构图
- **人物取位**:主体在竖屏画幅中的位置习惯、头部留白
- **字幕呈现**:台词字幕的位置、样式、强调方式(变色/放大)

## 声音印象
(依据画面与字幕可推断的部分:配乐出现的场合类型、音效强调点(如打脸音效)、\
旁白使用习惯;无法确认的标「?」)",
        merge_prompt: None,
        inputs: "[]",
        output_template: "09-视听语言.md",
        needs_video: true,
        user_input: false,
        params: r#"{}"#,
    },
    // ────────────────────── Stage B:分集资产(单集视频) ──────────────────────
    BuiltinSpec {
        id: "b-transcript",
        stage: "episode",
        sort_no: 10,
        name: "台词原文",
        scope: "per_episode",
        prompt: "\
本视频是《{drama_name}》第 {ep_no} 集。只做一件事:逐字精确转录全部台词。

规则:
1. 以画面中的字幕为准,逐字照抄——不改写、不润色、不总结、不遗漏,保留语气词与标点
2. 按出现顺序编号
3. 不要猜测说话人(说话人在后续环节另行标注)
4. 旁白/画外音照实转录,行首标「[旁白]」
5. 屏幕上出现的关键文字信息(短信、聊天记录、文件标题)转录并标「[画面文字]」

输出格式(每行一条):
1. 台词内容
2. [旁白] 旁白内容
3. [画面文字] 文字内容",
        merge_prompt: None,
        inputs: "[]",
        output_template: "分集/第{ep}集-台词原文.md",
        needs_video: true,
        user_input: false,
        // 逐字照抄是机械任务,深度思考纯烧钱(实测 high 档思考 token 占输出 94%)。
        params: r#"{"reasoning_effort":"low"}"#,
    },
    BuiltinSpec {
        id: "b-breakdown",
        stage: "episode",
        sort_no: 11,
        name: "拆解卡",
        scope: "per_episode",
        prompt: "\
本视频是《{drama_name}》第 {ep_no} 集(本集标题文案:{ep_title})。\
参考资料中有全剧人物档案与本集台词原文:人物一律使用档案中的名字,\
引用台词(尤其金句)必须逐字来自台词原文。只做一件事:输出本集拆解卡。

输出格式:

## 本集剧情
(100 字内)

## 场景列表
(每场一行:场景地点 | 出场人物 | 这场戏发生了什么)

## 本集功能
(从以下选择并说明,可多选:推进主线/埋设伏笔/情绪铺垫/打脸兑现/关系转折/过渡衔接)

## 开场钩子
(前 10 秒靠什么留住人:类型 + 一句话描述)

## 结尾钩子
- 类型:(悬念抛出/冲突升级/反转揭示/危机逼近/情感暴击)
- 强度:(1-5,5=不点下一集睡不着)
- 描述:(结尾定格在什么点上)

## 桥段
(本集用到的经典桥段,每条:桥段类型 | 大致时间点 | 一句话描述;\
常见类型:打脸、掀马甲、误会升级、英雄救美、当众羞辱、身份错认、以弱胜强、反派自爆等,不限于此)

## 情绪
- 主情绪:(爽/虐/悬/甜/怒)
- 强度:(1-5)
- 副情绪:(可选)

## 信息增量
(本集向观众新揭示了什么;若信息只对观众揭示而剧中人不知,标「观众视角」)

## 本集金句
(0-3 条冲击力强的原台词,没有就写「无」)",
        merge_prompt: None,
        inputs: r#"["a-characters","b-transcript"]"#,
        output_template: "分集/第{ep}集-拆解卡.md",
        needs_video: true,
        user_input: false,
        params: r#"{}"#,
    },
    // ────────────────────── Stage C:聚合资产(纯文本) ──────────────────────
    BuiltinSpec {
        id: "c-annotated",
        stage: "episode",
        sort_no: 12,
        name: "台词标注",
        scope: "per_episode",
        prompt: "\
本视频是《{drama_name}》第 {ep_no} 集。参考资料中有全剧人物档案与本集台词原文。\
只做一件事:对照台词原文,逐句标注说话人。

依据(按优先级):
1. 画面证据:谁在开口说话(口型、正反打镜头、景别指向)
2. 人物档案的外貌特征与姓名对应
3. 台词中的称呼与对话逻辑

规则:
- 保持台词原文与编号完全不变,只在前面加说话人
- 视觉与文本证据冲突时以画面为准
- 拿不准的说话人后加「?」,不要硬猜
- [旁白][画面文字] 保持原标注

输出格式(每行一条):
1. 说话人:台词内容
2. [旁白] 旁白内容",
        merge_prompt: None,
        inputs: r#"["a-characters","b-transcript"]"#,
        output_template: "分集/第{ep}集-台词标注.md",
        needs_video: true,
        user_input: false,
        params: r#"{}"#,
    },
    BuiltinSpec {
        id: "b-shotlist",
        stage: "episode",
        sort_no: 13,
        name: "分镜表",
        scope: "per_episode",
        prompt: "\
本视频是《{drama_name}》第 {ep_no} 集。参考资料中有本集台词原文(已编号)。\
只做一件事:输出本集逐镜头分镜表。

规则:
- 一个镜头 = 两次剪切之间的连续画面,按剪切点切分,不要合并
- 时间用「秒」,精确到 0.5 秒
- 台词列填台词原文的编号(如 3-5);无台词写「—」

输出格式(markdown 表格,每镜一行):

镜号 | 起-止(秒) | 景别 | 运镜 | 画面内容(谁在做什么,一句话) | 台词编号

表格之后追加一节:

## 本集镜头统计
- 镜头总数 / 平均镜头时长
- 景别分布(特写/近景/中景/全景 各多少个)",
        merge_prompt: None,
        inputs: r#"["b-transcript"]"#,
        output_template: "分集/第{ep}集-分镜表.md",
        needs_video: true,
        user_input: false,
        // 逐镜头记录是机械提取,低思考即可(同台词原文)。
        params: r#"{"reasoning_effort":"low"}"#,
    },
    BuiltinSpec {
        id: "c-scriptback",
        stage: "episode",
        sort_no: 14,
        name: "逆向剧本",
        scope: "per_episode",
        prompt: "\
只做一件事:把参考资料(本集拆解卡、台词标注、分镜表)合成为《{drama_name}》\
第 {ep_no} 集的标准分场剧本——从成片逆向还原「拍出这一集所用的剧本」,\
作为二创剧本的格式与容量基准。

规则:
- 台词逐字取自台词标注,不改写;说话人用台词标注中的名字
- 动作行(不含台词的画面行为)依据分镜表与拆解卡还原,简洁、可拍
- 场次以拆解卡场景列表为准;每场标注约时长(依据分镜表起止秒)

输出格式:

# 第{ep_no}集

## 1. 场景地点(内/外 · 日/夜)[约 XX 秒]

△ 动作行(人物动作、画面信息,一行一个动作)

人物名:台词
人物名:(神态/动作提示)台词

(依次每场……)",
        merge_prompt: None,
        inputs: r#"["b-breakdown","c-annotated","b-shotlist"]"#,
        output_template: "分集/第{ep}集-逆向剧本.md",
        needs_video: false,
        user_input: false,
        params: r#"{}"#,
    },
    BuiltinSpec {
        id: "c-profile",
        stage: "synth",
        sort_no: 0,
        name: "剧目档案",
        scope: "per_drama",
        prompt: "\
只做一件事:基于参考资料输出《{drama_name}》的剧目总档案。

各集标题文案(投放侧钩子,供题材判断参考):
{ep_titles}

输出格式:

# 《{drama_name}》剧目档案

## Logline
(一句话讲清这部剧:谁+困境+反转点)

## 题材标签
(多标签,如:逆袭/掀马甲/复仇/甜宠/重生……并注明主次)

## 核心爽点引擎
(观众为什么一直点下一集——这部剧反复兑现的核心爽感机制,说透)

## 目标观众画像
(性别/年龄/爽点偏好推断)

## 全剧梗概
(500 字内,完整讲一遍故事)

## 分幕概述
(按幕划分:集数范围 | 幕任务 | 结束于什么事件)",
        merge_prompt: None,
        inputs: r#"["a-characters","a-storyline","b-breakdown:all"]"#,
        output_template: "00-剧目档案.md",
        needs_video: false,
        user_input: false,
        params: r#"{}"#,
    },
    BuiltinSpec {
        id: "c-beatsheet",
        stage: "global",
        sort_no: 3,
        name: "节拍表",
        scope: "per_segment",
        prompt: "\
本视频是《{drama_name}》全剧完整拼接(共 {episode_count} 集)。各集时间范围:

{ep_timeline}

只做一件事:输出全剧节拍表 —— markdown 表格,每集一行,共 {episode_count} 行,列:

集 | 本集关键事件(1-2句) | 结尾钩子(类型/强度1-5) | 情绪(主情绪/强度1-5)

要求:{episode_count} 集一行不少,严禁用「……」「以下略」等方式省略;\
每集内容必须来自该集对应时间范围的画面;\
幕与幕的分界处,在表格中插入一行「── 第X幕结束 ──」。",
        merge_prompt: None,
        inputs: "[]",
        output_template: "03-节拍表.md",
        needs_video: true,
        user_input: false,
        params: r#"{}"#,
    },
    BuiltinSpec {
        id: "c-hooks",
        stage: "synth",
        sort_no: 4,
        name: "钩子链",
        scope: "per_drama",
        prompt: "\
只做一件事:基于参考资料分析《{drama_name}》的钩子链——短剧留存的命根子。

输出格式:

# 《{drama_name}》钩子链

## 逐集结尾钩子
(表格,每集一行:集 | 钩子类型 | 强度 | 一句话描述)

## 类型分布
(各类型出现次数与占比;开局 3 集、卡点前、结局前分别偏好什么类型)

## 排布规律
(强钩(4-5)的间隔节奏;连续弱钩最长几集;强钩前如何蓄势;\
可直接复用的排布公式,用「第N集类型」的抽象形式给出)

## 付费卡点推断
(依据钩子强度峰值与悬念密度,推断最可能的付费卡点集数区间及理由)",
        merge_prompt: None,
        inputs: r#"["c-beatsheet","b-breakdown:all"]"#,
        output_template: "04-钩子链.md",
        needs_video: false,
        user_input: false,
        params: r#"{}"#,
    },
    BuiltinSpec {
        id: "c-emotion",
        stage: "synth",
        sort_no: 5,
        name: "情绪曲线",
        scope: "per_drama",
        prompt: "\
只做一件事:基于参考资料输出《{drama_name}》的全剧情绪曲线。

输出格式:

# 《{drama_name}》情绪曲线

## 逐集情绪值
(表格,每集一行:集 | 主情绪 | 强度(1-5) | 副情绪;此表供程序绘图,严格保持格式)

## 曲线形态分析
(峰与谷的分布;虐-爽的交替节奏;最长的压抑蓄势段与其后的爆发;结尾走势)

## 情绪与钩子的配合
(情绪低谷时靠什么钩子留人;情绪峰值是否与强钩重合)",
        merge_prompt: None,
        inputs: r#"["b-breakdown:all"]"#,
        output_template: "05-情绪曲线.md",
        needs_video: false,
        user_input: false,
        params: r#"{}"#,
    },
    BuiltinSpec {
        id: "c-tropes",
        stage: "synth",
        sort_no: 6,
        name: "桥段库",
        scope: "per_drama",
        prompt: "\
只做一件事:基于参考资料汇总《{drama_name}》的桥段库。

输出格式:

# 《{drama_name}》桥段库

## 桥段总表
(按桥段类型分组;每条:集数 | 桥段描述 | 前置铺垫 | 兑现效果)

## 高频桥段
(出现 3 次以上的桥段类型:如何做出变化避免重复,每次升级了什么)

## 可复用桥段卡
(挑 5-10 个执行最好的桥段,每个写成可移植的「桥段卡」:\
### 桥段名 / 结构:铺垫→触发→兑现三步的抽象描述(不含本剧专有名词) / 适用场景)",
        merge_prompt: None,
        inputs: r#"["b-breakdown:all"]"#,
        output_template: "06-桥段库.md",
        needs_video: false,
        user_input: false,
        params: r#"{}"#,
    },
    BuiltinSpec {
        id: "c-quotes",
        stage: "synth",
        sort_no: 7,
        name: "金句台词",
        scope: "per_drama",
        prompt: "\
只做一件事:汇总《{drama_name}》金句库。以各集拆解卡的金句为候选,\
每条金句必须逐字核对台词原文后收录 —— 与原文不一致的以台词原文为准并修正。

输出格式:

# 《{drama_name}》金句台词

## 按功能分类
(分类:立人设/宣战撂狠话/打脸兑现/情感暴击/悬念挑逗;每条:集数 | 台词原文 | 使用情境)

## 句式提炼
(高频句式结构的抽象模板,如「你以为X,可惜Y」,每个模板附本剧用例)",
        merge_prompt: None,
        inputs: r#"["b-breakdown:all","b-transcript:all"]"#,
        output_template: "07-金句台词.md",
        needs_video: false,
        user_input: false,
        params: r#"{}"#,
    },
    BuiltinSpec {
        id: "c-voice",
        stage: "synth",
        sort_no: 8,
        name: "语言风格",
        scope: "per_drama",
        prompt: "\
只做一件事:基于参考资料(人物档案 + 全剧台词标注)为《{drama_name}》每个主要人物\
提炼语言风格卡——下游写新剧台词时靠它保持「人味」不跑偏。

输出格式(每个人物一节,按戏份排序):

## 人物名
- **整体风格**:一句话概括(如「绵里藏针,敬语当刀用」)
- **句式习惯**:句长偏好、常用句式结构(反问/命令/自嘲……)、语气烈度
- **口头禅与高频词**:逐字列出并注明使用场合;没有写「无」
- **称呼表**:对每个主要人物的称呼及其变化(关系变化前后不同要写明)
- **典型语例**:3-5 条最能代表该人物说话方式的台词,逐字引用并注明集数

最后追加一节:

## 对话风格总律
(全剧对话的共性:平均台词长度、信息密度、潜台词浓度、狠话/怼人的常用套路)",
        merge_prompt: None,
        inputs: r#"["a-characters","c-annotated:all"]"#,
        output_template: "10-语言风格.md",
        needs_video: false,
        user_input: false,
        params: r#"{}"#,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_specs_are_wellformed() {
        let ids: Vec<&str> = BUILTIN_SPECS.iter().map(|s| s.id).collect();
        assert_eq!(ids.len(), 16);
        for s in BUILTIN_SPECS {
            assert_eq!(
                ids.iter().filter(|i| **i == s.id).count(),
                1,
                "id 重复: {}",
                s.id
            );
            let deps: Vec<String> = serde_json::from_str(s.inputs)
                .unwrap_or_else(|e| panic!("{} inputs 非法: {e}", s.id));
            for d in &deps {
                let dep_id = d.strip_suffix(":all").unwrap_or(d);
                assert!(ids.contains(&dep_id), "{} 依赖不存在: {dep_id}", s.id);
            }
            if s.scope == "per_episode" {
                assert!(s.output_template.contains("{ep}"), "{} 模板缺 {{ep}}", s.id);
            }
            // 二创资产已下线(2026-07):素材包交下游自行分析改编方向,内置 spec 不再含 user_input。
            assert!(!s.user_input, "{} 不应为 user_input", s.id);
            let _: serde_json::Value = s
                .params
                .parse()
                .unwrap_or_else(|e| panic!("{} params 非法: {e}", s.id));
        }
    }
}
