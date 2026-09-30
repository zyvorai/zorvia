//! Durable operations API and the reconciler loop that drives them.
//! See `crate::operations`.

use super::*;
use crate::operations::{runtime, OpState, OperationsDb};

#[derive(Debug, Deserialize)]
pub struct ListOperationsQuery {
    kind: Option<String>,
    limit: Option<usize>,
}

pub async fn list_operations_handler(
    axum::extract::Query(q): axum::extract::Query<ListOperationsQuery>,
) -> impl IntoResponse {
    match OperationsDb::global().list(q.kind.as_deref(), q.limit.unwrap_or(50)) {
        Ok(ops) => Json(json!({ "operations": ops })).into_response(),
        Err(e) => {
            let (st, j) = err_json(
                500,
                "OPERATIONS_UNAVAILABLE",
                &sanitize_error(&e.to_string()),
            );
            (st, j).into_response()
        }
    }
}

pub async fn get_operation_handler(Path(id): Path<String>) -> impl IntoResponse {
    match OperationsDb::global().get(&id) {
        Ok(Some(op)) => Json(json!(op)).into_response(),
        Ok(None) => {
            let (st, j) = err_json(404, "NOT_FOUND", "operation not found");
            (st, j).into_response()
        }
        Err(e) => {
            let (st, j) = err_json(
                500,
                "OPERATIONS_UNAVAILABLE",
                &sanitize_error(&e.to_string()),
            );
            (st, j).into_response()
        }
    }
}

pub async fn cancel_operation_handler(Path(id): Path<String>) -> impl IntoResponse {
    match OperationsDb::global().request_cancel(&id) {
        Ok(Some(state)) => {
            let code = if state == OpState::Running { 202 } else { 200 };
            let status = StatusCode::from_u16(code).unwrap_or(StatusCode::OK);
            (status, Json(json!({ "id": id, "state": state }))).into_response()
        }
        Ok(None) => {
            let (st, j) = err_json(404, "NOT_FOUND", "operation not found");
            (st, j).into_response()
        }
        Err(e) => {
            let (st, j) = err_json(
                500,
                "OPERATIONS_UNAVAILABLE",
                &sanitize_error(&e.to_string()),
            );
            (st, j).into_response()
        }
    }
}

/// Reconciler loop. Only the lease leader acts; it first re-queues work a
/// previous process left running, then claims and runs queued operations.
pub fn spawn_operations_reconciler(state: SharedState) {
    tokio::spawn(async move {
        let mut recovered = false;
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            if crate::api::leader::skip_if_follower("operations") {
                continue;
            }
            if !recovered {
                runtime::recover();
                recovered = true;
            }
            let client = state.read().await.client();
            runtime::tick(&client).await;
        }
    });
}
