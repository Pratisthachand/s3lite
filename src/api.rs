use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use bytes::Bytes;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio_util::io::ReaderStream;
use tracing::{error, info};

use crate::{errors::{AppError, Result}, metadata::Metadata, storage::{FinalizeResult, Storage}};
use std::{path::PathBuf, sync::Arc};

#[derive(Clone)]
pub struct AppState {
    pub storage: Arc<Storage>,
    pub meta: Arc<Metadata>,
}

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

pub async fn upload_object(
    State(state): State<crate::AppState>,
    Query(params): Query<UploadParams>,
    mut body: axum::extract::BodyStream,
) -> Result<impl IntoResponse> {
    // stream to tmp file + compute hash
    let tmp = state.storage.create_temp().await.map_err(AppError::Internal)?;
    let mut hasher = Sha256::new();
    let mut size: u64 = 0;

    while let Some(chunk) = body.next().await {
        let bytes: Bytes = chunk.map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
        size += bytes.len() as u64;
        state.storage.write_all(tmp.as_path(), &bytes).await.map_err(AppError::Internal)?;
        hasher.update(&bytes);
    }

    let cid = hex::encode(hasher.finalize());
    let finalize = state.storage.finalize(tmp.clone(), &cid).await.map_err(AppError::Internal)?;

    let deduped = match finalize {
        FinalizeResult::Created(_) => false,
        FinalizeResult::AlreadyExisted(_) => true,
    };

    // Update metadata
    let deduped_from_meta = state.meta.upsert_object(&cid, size).map_err(AppError::Internal)?;
    // If file already existed on disk, deduped_from_meta should also be true on second put,
    // else first put returns false.

    // If provided, link name -> cid
    if let Some(name) = params.name {
        state.meta.link_name(&name, &cid).map_err(AppError::Internal)?;
    }

    Ok((StatusCode::CREATED, Json(UploadResponse { cid, size, deduped: deduped || deduped_from_meta })))
}

pub async fn get_object_by_cid(
    State(state): State<crate::AppState>,
    Path(cid): Path<String>,
) -> Result<impl IntoResponse> {
    if !state.storage.exists(&cid) {
        return Err(AppError::NotFound);
    }
    state.meta.inc_get().map_err(AppError::Internal)?;
    let path = state.storage.path_for_cid(&cid);
    let file = tokio::fs::File::open(&path).await.map_err(AppError::Internal)?;
    let stream = ReaderStream::new(file);
    let mut headers = HeaderMap::new();
    headers.insert(axum::http::header::ETAG, format!("\"sha256-{cid}\"").parse().unwrap());
    Ok((headers, axum::body::Body::from_stream(stream)))
}

pub async fn head_object_by_cid(
    State(state): State<crate::AppState>,
    Path(cid): Path<String>,
) -> Result<impl IntoResponse> {
    if !state.storage.exists(&cid) {
        return Err(AppError::NotFound);
    }
    let rec = state.meta.get_object(&cid).map_err(AppError::Internal)?;
    let mut headers = HeaderMap::new();
    if let Some(rec) = rec {
        headers.insert(axum::http::header::ETAG, format!("\"sha256-{}\"", rec.cid).parse().unwrap());
        headers.insert(axum::http::header::CONTENT_LENGTH, rec.size.into());
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
}

pub async fn get_metrics(State(state): State<crate::AppState>) -> Result<impl IntoResponse> {
    let s = state.meta.stats();
    let saved = s.logical_bytes as i64 - s.unique_bytes as i64;
    Ok(Json(Metrics {
        object_count: s.object_count,
        logical_bytes: s.logical_bytes,
        unique_bytes: s.unique_bytes,
        bytes_saved: saved,
        put_count: s.put_count,
        get_count: s.get_count,
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
    state.meta.link_name(&req.name, &req.cid).map_err(AppError::Internal)?;
    Ok((StatusCode::CREATED, Json(serde_json::json!({"name": req.name, "cid": req.cid}))))
}

pub async fn resolve_name(
    State(state): State<crate::AppState>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse> {
    match state.meta.resolve_name(&name).map_err(AppError::Internal)? {
        Some(cid) => Ok(Json(serde_json::json!({ "name": name, "cid": cid }))),
        None => Err(AppError::NotFound),
    }
}

pub async fn unlink_name(
    State(state): State<crate::AppState>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse> {
    let existed = state.meta.unlink_name(&name).map_err(AppError::Internal)?;
    if existed {
        Ok((StatusCode::NO_CONTENT, "".into_response()))
    } else {
        Err(AppError::NotFound)
    }
}