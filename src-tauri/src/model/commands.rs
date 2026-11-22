use crate::error::AppResult;
use crate::model::db;
use crate::model::types::{ModelDraft, ModelItem};
use crate::storage::Db;
use tauri::State;

#[tauri::command]
pub fn model_list(db: State<Db>) -> AppResult<Vec<ModelItem>> {
    let conn = db.conn();
    db::list(&conn)
}

#[tauri::command]
pub fn model_list_for_provider(db: State<Db>, provider_id: String) -> AppResult<Vec<ModelItem>> {
    let conn = db.conn();
    db::list_for_provider(&conn, &provider_id)
}

#[tauri::command]
pub fn model_add(db: State<'_, Db>, draft: ModelDraft) -> AppResult<String> {
    let conn = db.conn();
    db::add(&conn, &draft)
}

#[tauri::command]
pub fn model_delete(db: State<'_, Db>, id: String) -> AppResult<()> {
    let conn = db.conn();
    db::delete(&conn, &id)
}

#[tauri::command]
pub fn provider_model_add(
    db: State<'_, Db>,
    provider_id: String,
    model_id: String,
) -> AppResult<()> {
    let conn = db.conn();
    db::add_to_provider(&conn, &provider_id, &model_id)
}

#[tauri::command]
pub fn provider_model_remove(
    db: State<'_, Db>,
    provider_id: String,
    model_id: String,
) -> AppResult<()> {
    let conn = db.conn();
    db::remove_from_provider(&conn, &provider_id, &model_id)
}

#[tauri::command]
pub fn provider_model_set_default(
    db: State<'_, Db>,
    provider_id: String,
    model_id: String,
) -> AppResult<()> {
    let conn = db.conn();
    db::set_default(&conn, &provider_id, &model_id)
}
