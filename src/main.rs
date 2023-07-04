use core::fmt;

use axum::{
    self,
    extract::{Path, State},
    routing::get,
    Json, Router, response::IntoResponse,
};
use serde_json::json;
use sqlx::SqlitePool;
use autometrics::{autometrics, prometheus_exporter};

#[cfg(target_env = "musl")]
use mimalloc::MiMalloc;

#[cfg(target_env = "musl")]
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[derive(sqlx::FromRow, serde::Serialize)]
struct Word {
    word: String,
    definition: String,
    translation: String,
    phonetic: String,
}

#[tokio::main]
async fn main() {
    prometheus_exporter::init();


    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(16)
        .connect("./stardict.db?mode=ro")
        .await
        .expect("open sqlite db");

    let app = Router::new()
        .route("/", get(|| async { "REST API FOR ECDICT" }))
        .route("/exact/:word", get(handle_exact))
        .route("/fuzzy/:word", get(handle_fuzzy))
        .route("/metrics", get(|| async {
            prometheus_exporter::encode_http_response()
        }))
        .with_state(pool);

    axum::Server::bind(&"0.0.0.0:8000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}

#[autometrics(ok_if = Result::is_ok, track_concurrency)]
async fn handle_exact(
    State(pool): State<SqlitePool>,
    Path(word): Path<String>,
) -> Result<Word, EcdictError> {
    let mut conn = pool.acquire().await?;

    let word: Word = sqlx::query_as::<_, Word>("select * from stardict where word = ?")
        .bind(word)
        .fetch_one(&mut *conn)
        .await?;

    Ok(word)
}

#[autometrics(ok_if = Result::is_ok, track_concurrency)]
async fn handle_fuzzy(
    State(pool): State<SqlitePool>,
    Path(query): Path<String>,
) -> Result<Json<serde_json::Value>, EcdictError> {
    let mut conn = pool.acquire().await?;

    let query = query.replace('.', "%");

    let words = sqlx::query_as::<_, Word>("select * from stardict where word like ? limit 10")
        .bind(query)
        .fetch_all(&mut *conn)
        .await?;

    Ok(Json(json!(words)))
}


#[derive(Debug)]
enum EcdictError {
    DBError(sqlx::Error)
}
impl fmt::Display for EcdictError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
} 
impl std::error::Error for EcdictError {}

impl From<sqlx::Error> for EcdictError {
    fn from(value: sqlx::Error) -> Self {
        EcdictError::DBError(value)
    }
}

impl IntoResponse for EcdictError {
    fn into_response(self) -> axum::response::Response {
        Json(json!({"err": format!("{self:?}")})).into_response()
    }
}

impl IntoResponse for Word {
    fn into_response(self) -> axum::response::Response {
        Json(json!(self)).into_response()
    }
}

