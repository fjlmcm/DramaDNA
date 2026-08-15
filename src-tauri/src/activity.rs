// 管线「当前活动」状态行 —— 谁在干活谁更新,前端轮询展示,
// 让用户看见转码/上传/调用等无进度条阶段确实在执行。

use std::sync::{Mutex, OnceLock};
use std::time::Instant;

struct Activity {
    text: String,
    at: Instant,
}

fn cell() -> &'static Mutex<Activity> {
    static CELL: OnceLock<Mutex<Activity>> = OnceLock::new();
    CELL.get_or_init(|| {
        Mutex::new(Activity {
            text: String::new(),
            at: Instant::now(),
        })
    })
}

pub fn set(text: impl Into<String>) {
    let mut a = cell().lock().unwrap();
    a.text = text.into();
    a.at = Instant::now();
}

/// (状态文本, 距上次更新的秒数)。文本为空 = 尚无活动。
pub fn get() -> (String, u64) {
    let a = cell().lock().unwrap();
    (a.text.clone(), a.at.elapsed().as_secs())
}
