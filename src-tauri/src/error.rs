use serde::{Serialize, Serializer};

/// 应用统一错误类型。Tauri command 的 Err 必须可 Serialize。
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("数据库错误: {0}")]
    Db(#[from] sqlx::Error),

    #[error("{0}")]
    Msg(String),

    /// 用户主动取消 —— 单独类型,便于 commands 层把 runs.status 记为 cancelled 而非 failed。
    #[error("已取消")]
    Cancelled,
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
