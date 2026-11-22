use crate::provider::types::Protocol;
use serde::{Deserialize, Serialize};

/// 一等模型实体：协议归模型，不再绑在供应商上。
/// 同一 (protocol, model_name) 全局唯一，可被多家供应商复用（多对多）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Model {
    pub id: String,
    pub protocol: Protocol,
    pub model_name: String,
    pub label: String,
    /// 用户对能力表的覆写（JSON）。空 = 走内置推导值。
    #[serde(default)]
    pub capabilities_override: String,
    pub created_at: String,
}

/// 模型 + 运行时派生信息。capabilities 不入库，始终按 (protocol, model_name) 算，
/// 再叠加用户自己的覆写。
#[derive(Debug, Clone, Serialize)]
pub struct ModelItem {
    pub id: String,
    pub protocol: Protocol,
    pub model_name: String,
    pub label: String,
    pub capabilities: crate::capabilities::Capabilities,
    /// 挂了这个模型的供应商数量（前端据此提示"已被 N 家供应商使用"）
    pub provider_count: usize,
    /// 该模型是否挂在当前查看的供应商下（provider_list 的 model_ids 另给，这里备用）
    #[serde(default)]
    pub linked: bool,
    /// 该模型是否有用户能力覆写（前端据此显示"已自定义"）
    #[serde(default)]
    pub capabilities_override: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModelDraft {
    #[serde(default)]
    pub id: Option<String>,
    pub protocol: Protocol,
    pub model_name: String,
    #[serde(default)]
    pub label: String,
    /// 能力覆写 JSON（可选）。非法 JSON 在保存时报错，不静默丢弃。
    #[serde(default)]
    pub capabilities_override: String,
}
