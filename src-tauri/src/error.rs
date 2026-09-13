use serde::{ser::Serializer, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("数据库错误: {0}")]
    Db(String),
    #[error("网络请求失败: {0}")]
    Http(String),
    #[error("上游超时：{0}")]
    Timeout(String),
    /// 上游返回非 2xx（带状态码）：按状态码决定是否值得重试
    #[error("上游返回错误（HTTP {0}）：{1}")]
    Status(u16, String),
    /// 上游业务错误——明确拒绝（内容安全拦截、参数不合法等），重试无意义
    #[error("{0}")]
    Upstream(String),
    #[error("{0}")]
    General(String),
}

impl AppError {
    /// 是否值得重试。只重试"可能自愈"的错误：
    /// - `Http`：连接层抖动
    /// - `General`：上游响应结构异常（偶发）
    /// - `Status`：429 限流 / 5xx 服务端故障
    ///
    /// 不重试的一律是"再试也一样"的：4xx 业务错误、内容安全拦截（`Upstream`）、
    /// 超时（`Timeout`，请求已送达上游、图还在生成）。
    pub fn is_retryable(&self) -> bool {
        match self {
            AppError::Http(_) => true,
            AppError::General(_) => true,
            AppError::Status(code, _) => *code == 429 || *code >= 500,
            AppError::Timeout(_) => false,
            AppError::Upstream(_) => false,
            AppError::Db(_) => false,
        }
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        AppError::Db(e.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::General(e.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        // 超时单独成类：生成请求超时往往意味着上游仍在出图，
        // 调用方据此不重试（重试会重复扣费，并让上游并发叠加）
        if e.is_timeout() {
            AppError::Timeout(e.to_string())
        } else {
            AppError::Http(e.to_string())
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
