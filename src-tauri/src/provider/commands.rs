use crate::error::{AppError, AppResult};
use crate::provider::{client, db};
use crate::provider::db::SaveInput;
use crate::provider::types::*;
use crate::storage::Db;
use tauri::State;

#[tauri::command]
pub fn provider_list(db: State<Db>) -> AppResult<Vec<ProviderListItem>> {
    let conn = db.0.lock().unwrap();
    db::list(&conn)
}

#[tauri::command]
pub async fn provider_save(db: State<'_, Db>, provider: SaveInput) -> AppResult<()> {
    let conn = db.0.lock().unwrap();
    db::save(&conn, &provider)
}

#[tauri::command]
pub async fn provider_delete(db: State<'_, Db>, id: String) -> AppResult<()> {
    let conn = db.0.lock().unwrap();
    db::delete(&conn, &id)
}

/// 连通性测试。api_key 留空且给了已存供应商 id 时，使用库里已存的 Key。
#[tauri::command]
pub async fn provider_test(db: State<'_, Db>, draft: ProviderDraft) -> AppResult<TestResult> {
    let key = resolve_key(&db, &draft)?;
    Ok(client::test_connection(&draft, &key).await)
}

#[tauri::command]
pub async fn provider_discover_models(
    db: State<'_, Db>,
    draft: ProviderDraft,
) -> AppResult<DiscoveredModels> {
    let key = resolve_key(&db, &draft)?;
    client::discover_models(&draft, &key).await
}

fn resolve_key(db: &State<Db>, draft: &ProviderDraft) -> AppResult<String> {
    if !draft.api_key.trim().is_empty() {
        return Ok(draft.api_key.trim().to_string());
    }
    if let Some(id) = &draft.id {
        let conn = db.0.lock().unwrap();
        if let Some(k) = db::get_key(&conn, id)? {
            return Ok(k);
        }
    }
    Err(AppError::General("请先填写 API Key".into()))
}
