use axum::{
    extract::{Path, Query, State},
    body::Bytes,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::IntoResponse,
    Json,
};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio_util::io::ReaderStream;

use crate::{
    AppState, errors::{AppError, Result}, metadata::Metadata, storage::{FinalizeResult, Storage}
};
use tokio::io::AsyncWriteExt;

#[derive(Deserialize)]
pub struct UploadParams {
    pub name: Option<String>,
}

#[derive(Serialize)]
pub struct UploadResponse {
    pub cid: String,
    pub size: u64,
    pub deduped: bool,
}

pub async fn get_health() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok" }))
}

// Handles large files by processing chunks as they arrive (not loading all into memory)
pub async fn upload_object_stream(
    State(state): State<AppState>,
    body: axum::body::Body,
) -> Result<Json<serde_json::Value>> {
    // Step 1: Convert body to bytes
    let bytes = axum::body::to_bytes(body, usize::MAX)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
    
    println!("✓ Received {} bytes", bytes.len());
    
    // Step 2: Create a temporary file
    let temp_path = state.storage.create_temp()
        .await
        .map_err(|e| AppError::Internal(e))?;
    
    println!("✓ Created temp file: {:?}", temp_path);
    
    // Step 3: Calculate SHA256 hash while writing file
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    
    // Write bytes to temp file
    let mut file = tokio::fs::File::create(&temp_path)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
    
    file.write_all(&bytes)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
    
    file.flush()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
    
    // Step 4: Get the final hash as a hex string
    let hash_result = hasher.finalize();
    let cid = format!("{:x}", hash_result);
    
    println!("✓ Content ID (CID): {}", cid);
    
    // Step 5: Move temp file to permanent storage
    let final_result = state.storage.finalize(temp_path, &cid)
        .await
        .map_err(|e| AppError::Internal(e))?;
    
    // Step 6: Check if this was a new file or a duplicate
    let was_duplicate = matches!(final_result, FinalizeResult::AlreadyExisted(_));
    
    if was_duplicate {
        println!("Duplicate detected!");
    } else {
        println!("New unique file saved!");
    }
    
    // Step 7: Update metadata database
    let total_bytes = bytes.len() as u64;
    state.meta.inc_put(
        total_bytes,
        if was_duplicate { 0 } else { total_bytes },
        !was_duplicate
    )
        .map_err(|e| AppError::Internal(e))?;
    
    // Step 8: Return success response
    Ok(Json(serde_json::json!({
        "cid": cid,
        "size": total_bytes,
        "duplicate": was_duplicate
    })))
}

pub async fn get_object_by_cid(
    State(state): State<crate::AppState>,
    Path(cid): Path<String>,
) -> Result<impl IntoResponse> {
    if !state.storage.exists(&cid) {
        return Err(AppError::NotFound);
    }
    state.meta.inc_get().map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
    let path = state.storage.path_for_cid(&cid);
    let file = tokio::fs::File::open(&path).await
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
    let stream = ReaderStream::new(file);

    let mut headers = HeaderMap::new();
    headers.insert(header::ETAG, HeaderValue::from_str(&format!("\"sha256-{cid}\"")).unwrap());

    // Axum 0.7: use Body::from_stream
    let body = axum::body::Body::from_stream(stream);
    Ok((headers, body))
}

pub async fn head_object_by_cid(
    State(state): State<crate::AppState>,
    Path(cid): Path<String>,
) -> Result<impl IntoResponse> {
    if !state.storage.exists(&cid) {
        return Err(AppError::NotFound);
    }
    let rec = state.meta.get_object(&cid)
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
    let mut headers = HeaderMap::new();
    if let Some(rec) = rec {
        headers.insert(header::ETAG, HeaderValue::from_str(&format!("\"sha256-{}\"", rec.cid)).unwrap());
        headers.insert(header::CONTENT_LENGTH, HeaderValue::from_str(&rec.size.to_string()).unwrap());
    }
    Ok((StatusCode::OK, headers))
}

#[derive(Serialize)]
pub struct Metrics {
    object_count: u64,
    logical_bytes: u64,
    unique_bytes: u64,
    bytes_saved: i64,
    put_count: u64,
    get_count: u64,
    savings_percentage: String,
}

pub async fn get_metrics(State(state): State<crate::AppState>) -> Result<impl IntoResponse> {
    let s = state.meta.stats();
    let saved = s.logical_bytes as i64 - s.unique_bytes as i64;

    // Calculate savings percentage
    let savings_percentage = if s.logical_bytes > 0 {
        let pct = ((s.logical_bytes - s.unique_bytes) as f64 / s.logical_bytes as f64) * 100.0;
        format!("{:.1}%", pct)
    } else {
        "0.0%".to_string()
    };

    Ok(Json(Metrics {
        object_count: s.object_count,
        logical_bytes: s.logical_bytes,
        unique_bytes: s.unique_bytes,
        bytes_saved: saved,
        put_count: s.put_count,
        get_count: s.get_count,
        savings_percentage,
    }))
}

#[derive(Deserialize)]
pub struct LinkReq {
    pub name: String,
    pub cid: String,
}

pub async fn link_name(
    State(state): State<crate::AppState>,
    Json(req): Json<LinkReq>,
) -> Result<impl IntoResponse> {
    if !state.storage.exists(&req.cid) {
        return Err(AppError::BadRequest("CID does not exist".into()));
    }
    state.meta.link_name(&req.name, &req.cid)
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
    Ok((StatusCode::CREATED, Json(serde_json::json!({"name": req.name, "cid": req.cid}))))
}

pub async fn resolve_name(
    State(state): State<crate::AppState>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse> {
    match state.meta.resolve_name(&name).map_err(|e| AppError::Internal(anyhow::anyhow!(e)))? {
        Some(cid) => Ok(Json(serde_json::json!({ "name": name, "cid": cid }))),
        None => Err(AppError::NotFound),
    }
}

pub async fn unlink_name(
    State(state): State<crate::AppState>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse> {
    let existed = state.meta.unlink_name(&name).map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
    if existed {
        Ok((StatusCode::NO_CONTENT, "".into_response()))
    } else {
        Err(AppError::NotFound)
    }
}

// Returns HTML page showing real-time storage statistics
pub async fn dashboard() -> axum::response::Html<&'static str> {
    axum::response::Html(include_str!("../static/dashboard.html"))
}