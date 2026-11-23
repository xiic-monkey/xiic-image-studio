use crate::error::{AppError, AppResult};
use crate::model::types::{Model, ModelDraft, ModelItem};
use crate::provider::types::Protocol;
use rusqlite::{params, Connection, OptionalExtension};

const COLS: &str = "id, protocol, model_name, label, capabilities_json, created_at";

fn row_to_model(row: &rusqlite::Row) -> rusqlite::Result<Model> {
    let protocol_str: String = row.get(1)?;
    Ok(Model {
        id: row.get(0)?,
        protocol: Protocol::parse(&protocol_str).unwrap_or(Protocol::OpenAiImages),
        model_name: row.get(2)?,
        label: row.get(3)?,
        capabilities_override: row.get(4)?,
        created_at: row.get(5)?,
    })
}

/// 推导能力并叠加用户覆写。覆写非法时按「没覆写」处理——
/// 能力表只是为了让参数发得出去，不该因为一段坏 JSON 就挡住出图。
pub(crate) fn caps_with_override(
    protocol: Protocol,
    model_name: &str,
    override_json: &str,
) -> crate::capabilities::Capabilities {
    let mut caps = crate::capabilities::for_model(protocol, model_name);
    if let Some(o) = crate::capabilities::parse_override(Some(override_json)) {
        caps.apply_override(&o);
    }
    caps
}

fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// 全局模型列表。带运行时能力与被引用的供应商数。
pub fn list(conn: &Connection) -> AppResult<Vec<ModelItem>> {
    let mut stmt = conn.prepare(
        "SELECT m.id, m.protocol, m.model_name, m.label, m.capabilities_json, m.created_at,
                (SELECT COUNT(*) FROM provider_models pm WHERE pm.model_id = m.id) AS pc
         FROM models m ORDER BY m.created_at ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        let model = Model {
            id: row.get(0)?,
            protocol: Protocol::parse(&row.get::<_, String>(1)?).unwrap_or(Protocol::OpenAiImages),
            model_name: row.get(2)?,
            label: row.get(3)?,
            capabilities_override: row.get(4)?,
            created_at: row.get(5)?,
        };
        let provider_count: i64 = row.get(6)?;
        Ok((model, provider_count as usize))
    })?;
    let mut out = Vec::new();
    for item in rows {
        let (model, provider_count) = item?;
        let capabilities = caps_with_override(
            model.protocol,
            &model.model_name,
            &model.capabilities_override,
        );
        out.push(ModelItem {
            id: model.id,
            protocol: model.protocol,
            model_name: model.model_name,
            label: model.label,
            capabilities,
            provider_count,
            linked: false,
            capabilities_override: model.capabilities_override,
        });
    }
    drop(stmt);
    Ok(out)
}

/// 某供应商下挂的模型，默认模型排最前。
pub fn list_for_provider(conn: &Connection, provider_id: &str) -> AppResult<Vec<ModelItem>> {
    let mut stmt = conn.prepare(
        "SELECT m.id, m.protocol, m.model_name, m.label, m.capabilities_json, m.created_at, pm.is_default,
                (SELECT COUNT(*) FROM provider_models pm2 WHERE pm2.model_id = m.id) AS pc
         FROM models m
         JOIN provider_models pm ON pm.model_id = m.id AND pm.provider_id = ?1
         ORDER BY pm.is_default DESC, m.created_at ASC",
    )?;
    let rows = stmt.query_map(params![provider_id], |row| {
        let model = Model {
            id: row.get(0)?,
            protocol: Protocol::parse(&row.get::<_, String>(1)?).unwrap_or(Protocol::OpenAiImages),
            model_name: row.get(2)?,
            label: row.get(3)?,
            capabilities_override: row.get(4)?,
            created_at: row.get(5)?,
        };
        let _is_default: i64 = row.get(6)?;
        let provider_count: i64 = row.get(7)?;
        Ok((model, provider_count as usize))
    })?;
    let mut out = Vec::new();
    for item in rows {
        let (model, provider_count) = item?;
        let capabilities = caps_with_override(
            model.protocol,
            &model.model_name,
            &model.capabilities_override,
        );
        out.push(ModelItem {
            id: model.id,
            protocol: model.protocol,
            model_name: model.model_name,
            label: model.label,
            capabilities,
            provider_count,
            linked: true,
            capabilities_override: model.capabilities_override,
        });
    }
    Ok(out)
}

/// 生成请求解析后的模型：id / 协议 / 上游模型名 / 运行时能力。
/// GUI 与 CLI 共用，避免两处各自解析导致行为漂移。
#[derive(Debug, Clone)]
pub struct ResolvedModel {
    pub id: String,
    pub protocol: Protocol,
    pub model_name: String,
    pub capabilities: crate::capabilities::Capabilities,
}

/// 按供应商 + 可选 model_id 解析出实际使用的模型（含能力）。
/// 这是 GUI / CLI 提交前共用的唯一入口：model_id 为空回退默认模型，
/// 指定但未关联则报错，绝不静默改用别的模型。
pub fn resolve(
    conn: &Connection,
    provider_id: &str,
    model_id: Option<&str>,
) -> AppResult<ResolvedModel> {
    let (id, protocol, model_name, override_json) =
        resolve_for_request(conn, provider_id, model_id)?;
    let capabilities = caps_with_override(protocol, &model_name, &override_json);
    Ok(ResolvedModel {
        id,
        protocol,
        model_name,
        capabilities,
    })
}

pub fn get(conn: &Connection, id: &str) -> AppResult<Option<Model>> {
    let mut stmt = conn.prepare(&format!("SELECT {COLS} FROM models WHERE id = ?1"))?;
    Ok(stmt.query_row(params![id], row_to_model).optional()?)
}

/// 新增或复用一个 (protocol, model_name) 模型，返回其 id。
/// 用 INSERT OR IGNORE + 反查实现"存在即复用"（与迁移同模式，兼容并发）。
pub fn ensure_model(conn: &Connection, protocol: Protocol, model_name: &str) -> AppResult<String> {
    let id = new_id();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT OR IGNORE INTO models (id, protocol, model_name, label, capabilities_json, created_at)
         VALUES (?1, ?2, ?3, '', '', ?4)",
        params![id, protocol.as_str(), model_name, now],
    )?;
    let resolved: String = conn.query_row(
        "SELECT id FROM models WHERE protocol = ?1 AND model_name = ?2",
        params![protocol.as_str(), model_name],
        |r| r.get(0),
    )?;
    Ok(resolved)
}

pub fn add(conn: &Connection, draft: &ModelDraft) -> AppResult<String> {
    let model_name = draft.model_name.trim();
    if model_name.is_empty() {
        return Err(AppError::General("模型名不能为空".into()));
    }
    let id = ensure_model(conn, draft.protocol, model_name)?;
    // 用户可能补了 label，更新之（幂等：重复提交也不报错）
    if !draft.label.trim().is_empty() {
        conn.execute(
            "UPDATE models SET label = ?2 WHERE id = ?1",
            params![id, draft.label.trim()],
        )?;
    }
    // 能力覆写：None = 前端没提这件事（保持原值）；Some("") = 显式清除；
    // 非空则校验后写入。区分两者才能让「留空=恢复内置推导」真的生效。
    match draft.capabilities_override.as_deref().map(str::trim) {
        Some(raw) if !raw.is_empty() => {
            // 覆写非法就直接报错：用户以为自己填上了，实际没生效才是最糟的
            if serde_json::from_str::<crate::capabilities::CapabilityOverride>(raw).is_err() {
                return Err(AppError::General(
                    "能力覆写不是合法 JSON，字段可用：sizes / quality / seed / refs / mask / endpoint / sizeLabel（或 size_label）"
                        .into(),
                ));
            }
            conn.execute(
                "UPDATE models SET capabilities_json = ?2 WHERE id = ?1",
                params![id, raw],
            )?;
        }
        Some(_) => {
            conn.execute(
                "UPDATE models SET capabilities_json = '' WHERE id = ?1",
                params![id],
            )?;
        }
        None => {}
    }
    Ok(id)
}

pub fn delete(conn: &Connection, id: &str) -> AppResult<()> {
    // provider_models 有 ON DELETE CASCADE，关联会自动清掉
    conn.execute("DELETE FROM models WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn add_to_provider(conn: &Connection, provider_id: &str, model_id: &str) -> AppResult<()> {
    // 校验两端都存在，给出可读错误（否则外键报错对前端不友好）
    if get(conn, model_id)?.is_none() {
        return Err(AppError::General("模型不存在".into()));
    }
    let exists: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM providers WHERE id = ?1",
        [provider_id],
        |r| r.get(0),
    )?;
    if !exists {
        return Err(AppError::General("供应商不存在".into()));
    }
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT OR IGNORE INTO provider_models (provider_id, model_id, is_default, created_at)
         VALUES (?1, ?2, 0, ?3)",
        params![provider_id, model_id, now],
    )?;
    // 供应商的第一个模型自动设为默认
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM provider_models WHERE provider_id = ?1",
        [provider_id],
        |r| r.get(0),
    )?;
    if n == 1 {
        set_default(conn, provider_id, model_id)?;
    }
    Ok(())
}

pub fn remove_from_provider(conn: &Connection, provider_id: &str, model_id: &str) -> AppResult<()> {
    conn.execute(
        "DELETE FROM provider_models WHERE provider_id = ?1 AND model_id = ?2",
        params![provider_id, model_id],
    )?;
    // 删掉的是默认模型时，把剩下的第一个顶上
    let default_left: i64 = conn.query_row(
        "SELECT COUNT(*) FROM provider_models WHERE provider_id = ?1 AND is_default = 1",
        [provider_id],
        |r| r.get(0),
    )?;
    if default_left == 0 {
        let next: Option<String> = conn
            .query_row(
                "SELECT model_id FROM provider_models WHERE provider_id = ?1 ORDER BY created_at ASC LIMIT 1",
                [provider_id],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(id) = next {
            set_default(conn, provider_id, &id)?;
        }
    }
    Ok(())
}

pub fn set_default(conn: &Connection, provider_id: &str, model_id: &str) -> AppResult<()> {
    // 校验该模型确实挂在供应商下，否则不该设为默认
    let linked: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM provider_models WHERE provider_id = ?1 AND model_id = ?2",
        params![provider_id, model_id],
        |r| r.get(0),
    )?;
    if !linked {
        return Err(AppError::General("该模型未挂在此供应商下".into()));
    }
    conn.execute_batch("BEGIN")?;
    let result = (|| -> AppResult<()> {
        conn.execute(
            "UPDATE provider_models SET is_default = 0 WHERE provider_id = ?1",
            [provider_id],
        )?;
        conn.execute(
            "UPDATE provider_models SET is_default = 1 WHERE provider_id = ?1 AND model_id = ?2",
            params![provider_id, model_id],
        )?;
        Ok(())
    })();
    match result {
        Ok(()) => conn.execute_batch("COMMIT")?,
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            return Err(e);
        }
    }
    Ok(())
}

/// 生成请求解析：给定供应商 + 可选模型标识，返回实际使用的
/// (model_id, protocol, model_name, capabilities_override)。
/// 标识可以是模型 id 或模型名（与 CLI `export --model` 按名字过滤保持一致）；
/// 为空时回退到供应商的默认模型；指定了但没挂在该供应商下则报错——
/// 不能静默改用别的模型，那会让用户以为在用 A 实际跑的是 B。
pub fn resolve_for_request(
    conn: &Connection,
    provider_id: &str,
    model_id: Option<&str>,
) -> AppResult<(String, Protocol, String, String)> {
    let row = match model_id {
        Some(mid) if !mid.trim().is_empty() => conn
            .query_row(
                "SELECT m.id, m.protocol, m.model_name, m.capabilities_json
                 FROM models m
                 JOIN provider_models pm ON pm.model_id = m.id
                 WHERE pm.provider_id = ?1 AND (m.id = ?2 OR m.model_name = ?2)
                 ORDER BY CASE WHEN m.id = ?2 THEN 0 ELSE 1 END, m.created_at ASC LIMIT 1",
                params![provider_id, mid],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| {
                AppError::General("所选模型未挂在此供应商下，请先在供应商页关联".into())
            })?,
        _ => conn
            .query_row(
                "SELECT m.id, m.protocol, m.model_name, m.capabilities_json
                 FROM models m
                 JOIN provider_models pm ON pm.model_id = m.id
                 WHERE pm.provider_id = ?1
                 ORDER BY pm.is_default DESC, m.created_at ASC LIMIT 1",
                [provider_id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| {
                AppError::General("该供应商还没有关联模型，请先在供应商页添加模型".into())
            })?,
    };
    let protocol = Protocol::parse(&row.1)
        .ok_or_else(|| AppError::General(format!("模型 {} 的协议 {} 无法识别", row.0, row.1)))?;
    Ok((row.0, protocol, row.2, row.3))
}

/// 该供应商的模型 id 列表（前端 provider_list 用）。
pub fn model_ids_for_provider(conn: &Connection, provider_id: &str) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT model_id FROM provider_models WHERE provider_id = ?1 ORDER BY is_default DESC, created_at ASC",
    )?;
    let ids = stmt
        .query_map([provider_id], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::init;

    fn conn() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        init(&c).unwrap();
        // init() 在空库上写了迁移标记；测试里不需要迁移，保持即可
        c
    }

    fn provider(conn: &Connection, id: &str) {
        conn.execute(
            "INSERT INTO providers (id, name, base_url, protocol, model, custom_headers, api_key, created_at, updated_at)
             VALUES (?1, ?2, 'https://x', 'openai_images', '', '{}', '', 'now', 'now')",
            params![id, id],
        )
        .unwrap();
    }

    #[test]
    fn ensure_model_reuses_same_protocol_and_name() {
        let c = conn();
        let a = ensure_model(&c, Protocol::OpenAiImages, "gpt-image-1").unwrap();
        let b = ensure_model(&c, Protocol::OpenAiImages, "gpt-image-1").unwrap();
        assert_eq!(a, b);
        // 不同协议是不同模型
        let c2 = ensure_model(&c, Protocol::OpenAiChatImage, "gpt-image-1").unwrap();
        assert_ne!(a, c2);
        let n: i64 = c
            .query_row("SELECT COUNT(*) FROM models", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 2);
    }

    #[test]
    fn one_model_can_link_to_multiple_providers() {
        let c = conn();
        provider(&c, "p1");
        provider(&c, "p2");
        let m = add(
            &c,
            &ModelDraft {
                id: None,
                protocol: Protocol::OpenAiImages,
                model_name: "gpt-image-1".into(),
                label: "GPT Image".into(),
                capabilities_override: None,
            },
        )
        .unwrap();
        add_to_provider(&c, "p1", &m).unwrap();
        add_to_provider(&c, "p2", &m).unwrap();
        let items = list(&c).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].provider_count, 2, "同一模型应被两家供应商复用");
        assert_eq!(list_for_provider(&c, "p1").unwrap().len(), 1);
        assert_eq!(list_for_provider(&c, "p2").unwrap().len(), 1);
    }

    #[test]
    fn provider_can_have_multiple_models_with_one_default() {
        let c = conn();
        provider(&c, "p1");
        let m1 = add(
            &c,
            &ModelDraft {
                id: None,
                protocol: Protocol::OpenAiImages,
                model_name: "gpt-image-1".into(),
                label: "".into(),
                capabilities_override: None,
            },
        )
        .unwrap();
        let m2 = add(
            &c,
            &ModelDraft {
                id: None,
                protocol: Protocol::GeminiNative,
                model_name: "nano-banana".into(),
                label: "".into(),
                capabilities_override: None,
            },
        )
        .unwrap();
        add_to_provider(&c, "p1", &m1).unwrap();
        add_to_provider(&c, "p1", &m2).unwrap();
        let (rid, proto, name, _) = resolve_for_request(&c, "p1", None).unwrap();
        assert_eq!(rid, m1, "第一个模型自动成为默认");
        assert_eq!(proto, Protocol::OpenAiImages);
        assert_eq!(name, "gpt-image-1");
        set_default(&c, "p1", &m2).unwrap();
        let (rid, proto, _, _) = resolve_for_request(&c, "p1", None).unwrap();
        assert_eq!(rid, m2);
        assert_eq!(proto, Protocol::GeminiNative);
    }

    #[test]
    fn resolve_accepts_model_name_as_well_as_id() {
        let c = conn();
        provider(&c, "p1");
        let m = add(
            &c,
            &ModelDraft {
                id: None,
                protocol: Protocol::OpenAiImages,
                model_name: "gpt-image-1".into(),
                label: "".into(),
                capabilities_override: None,
            },
        )
        .unwrap();
        add_to_provider(&c, "p1", &m).unwrap();
        // 传模型名也能解析到同一条模型（CLI export --model 也按名字）
        let (rid, _, name, _) = resolve_for_request(&c, "p1", Some("gpt-image-1")).unwrap();
        assert_eq!(rid, m);
        assert_eq!(name, "gpt-image-1");
        let (rid2, _, _, _) = resolve_for_request(&c, "p1", Some(&m)).unwrap();
        assert_eq!(rid2, m);
    }

    #[test]
    fn resolve_rejects_model_not_linked_to_provider() {
        let c = conn();
        provider(&c, "p1");
        provider(&c, "p2");
        let m = add(
            &c,
            &ModelDraft {
                id: None,
                protocol: Protocol::OpenAiImages,
                model_name: "gpt-image-1".into(),
                label: "".into(),
                capabilities_override: None,
            },
        )
        .unwrap();
        add_to_provider(&c, "p2", &m).unwrap();
        let err = resolve_for_request(&c, "p1", Some(&m)).unwrap_err();
        assert!(err.to_string().contains("未挂在此供应商下"), "实际：{err}");
    }

    #[test]
    fn resolve_with_no_model_errors_when_provider_has_none() {
        let c = conn();
        provider(&c, "p1");
        let err = resolve_for_request(&c, "p1", None).unwrap_err();
        assert!(err.to_string().contains("还没有关联模型"), "实际：{err}");
    }

    #[test]
    fn capability_override_replaces_builtin_sizes() {
        let c = conn();
        provider(&c, "p1");
        let m = add(
            &c,
            &ModelDraft {
                id: None,
                protocol: Protocol::OpenAiImages,
                model_name: "my-relay-model".into(),
                label: "".into(),
                capabilities_override: Some(r#"{"sizes":["512x512"],"seed":true}"#.into()),
            },
        )
        .unwrap();
        add_to_provider(&c, "p1", &m).unwrap();

        let resolved = resolve(&c, "p1", Some(&m)).unwrap();
        assert!(
            resolved.capabilities.allows_size("512x512"),
            "覆写尺寸应生效"
        );
        assert!(
            !resolved.capabilities.allows_size("1024x1024"),
            "覆写后内置尺寸表应被替换"
        );
        assert!(resolved.capabilities.seed, "覆写可开启 seed");

        let items = list(&c).unwrap();
        assert!(
            items[0].capabilities.allows_size("512x512"),
            "模型列表也要带覆写"
        );
        assert!(!items[0].capabilities_override.is_empty());
    }

    #[test]
    fn invalid_capability_override_is_rejected_on_save() {
        let c = conn();
        let err = add(
            &c,
            &ModelDraft {
                id: None,
                protocol: Protocol::OpenAiImages,
                model_name: "broken-model".into(),
                label: "".into(),
                capabilities_override: Some("{不是json".into()),
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("合法 JSON"), "实际：{err}");
    }

    #[test]
    fn empty_override_clears_previous_override() {
        let c = conn();
        add(
            &c,
            &ModelDraft {
                id: None,
                protocol: Protocol::OpenAiImages,
                model_name: "my-relay-model".into(),
                label: "".into(),
                capabilities_override: Some(r#"{"sizes":["512x512"]}"#.into()),
            },
        )
        .unwrap();
        assert!(!list(&c).unwrap()[0].capabilities_override.is_empty());

        // 界面「留空=恢复内置推导」必须真的清掉，而不是保留上一次的 JSON
        add(
            &c,
            &ModelDraft {
                id: None,
                protocol: Protocol::OpenAiImages,
                model_name: "my-relay-model".into(),
                label: "".into(),
                capabilities_override: Some("".into()),
            },
        )
        .unwrap();
        let items = list(&c).unwrap();
        assert!(items[0].capabilities_override.is_empty(), "覆写应被清除");
        assert!(items[0].capabilities.allows_size("1024x1024"));
    }

    #[test]
    fn removing_default_promotes_next_model() {
        let c = conn();
        provider(&c, "p1");
        let m1 = add(
            &c,
            &ModelDraft {
                id: None,
                protocol: Protocol::OpenAiImages,
                model_name: "gpt-image-1".into(),
                label: "".into(),
                capabilities_override: None,
            },
        )
        .unwrap();
        let m2 = add(
            &c,
            &ModelDraft {
                id: None,
                protocol: Protocol::GeminiNative,
                model_name: "nano-banana".into(),
                label: "".into(),
                capabilities_override: None,
            },
        )
        .unwrap();
        add_to_provider(&c, "p1", &m1).unwrap();
        add_to_provider(&c, "p1", &m2).unwrap();
        remove_from_provider(&c, "p1", &m1).unwrap();
        let (rid, _, _, _) = resolve_for_request(&c, "p1", None).unwrap();
        assert_eq!(rid, m2, "删掉默认模型后剩下的应自动成为默认");
    }
}
