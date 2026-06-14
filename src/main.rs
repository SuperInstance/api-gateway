use axum::{
    extract::{Path, Query, State},
    http::{Method, Request, StatusCode, Uri},
    middleware::{self, Next},
    response::{IntoResponse, Json, Response},
    routing::{any, get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

// ─── Types ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TernaryRoute {
    Accept,   // +1: route to this upstream
    Neutral,  // 0:  no preference
    Reject,   // -1: never route here
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Upstream {
    pub id: String,
    pub url: String,
    pub weight: f64,
    pub priority: u32,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    pub path: String,
    pub methods: Vec<String>,
    pub upstreams: Vec<Upstream>,
    pub ternary_preference: TernaryRoute,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteTable {
    pub routes: Vec<Route>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub uptime_secs: u64,
    pub total_requests: u64,
    pub active_routes: usize,
}

#[derive(Debug, Serialize)]
pub struct MetricsSnapshot {
    pub total_requests: u64,
    pub total_errors: u64,
    pub avg_latency_ms: f64,
    pub routes: Vec<RouteMetrics>,
}

#[derive(Debug, Serialize)]
pub struct RouteMetrics {
    pub path: String,
    pub hits: u64,
    pub errors: u64,
    pub avg_latency_ms: f64,
}

// ─── App State ──────────────────────────────────────────────────────

#[derive(Clone)]
struct AppState {
    route_table: Arc<RouteTable>,
    metrics: Arc<RwLock<ApiMetrics>>,
    start_time: Instant,
}

#[derive(Default)]
struct ApiMetrics {
    total_requests: u64,
    total_errors: u64,
    total_latency_ms: f64,
    route_metrics: HashMap<String, RouteMetrics>,
}

// ─── Handlers ───────────────────────────────────────────────────────

async fn health_check(State(state): State<AppState>) -> Json<HealthResponse> {
    let metrics = state.metrics.read().await;
    Json(HealthResponse {
        status: "healthy".into(),
        uptime_secs: state.start_time.elapsed().as_secs(),
        total_requests: metrics.total_requests,
        active_routes: state.route_table.routes.len(),
    })
}

async fn get_metrics(State(state): State<AppState>) -> Json<MetricsSnapshot> {
    let metrics = state.metrics.read().await;
    let avg = if metrics.total_requests > 0 {
        metrics.total_latency_ms / metrics.total_requests as f64
    } else {
        0.0
    };
    Json(MetricsSnapshot {
        total_requests: metrics.total_requests,
        total_errors: metrics.total_errors,
        avg_latency_ms: avg,
        routes: metrics.route_metrics.values().cloned().collect(),
    })
}

async fn get_routes(State(state): State<AppState>) -> Json<Vec<Route>> {
    Json(state.route_table.routes.clone())
}

async fn proxy_handler(
    State(state): State<AppState>,
    req: Request<axum::body::Body>,
) -> Response {
    let path = req.uri().path().to_string();
    let start = Instant::now();

    // Find matching route
    let upstream = state.route_table.routes.iter()
        .find(|r| r.path == "*" || path.starts_with(&r.path))
        .and_then(|r| r.upstreams.first());

    match upstream {
        Some(upstream) => {
            let client = reqwest::Client::new();
            let upstream_url = format!("{}{}", upstream.url, path);

            match client.get(&upstream_url).send().await {
                Ok(resp) => {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    let latency = start.elapsed().as_millis() as f64;

                    // Record metrics
                    let mut metrics = state.metrics.write().await;
                    metrics.total_requests += 1;
                    metrics.total_latency_ms += latency;
                    metrics.route_metrics.entry(path.clone())
                        .or_insert(RouteMetrics {
                            path: path.clone(),
                            hits: 0,
                            errors: 0,
                            avg_latency_ms: 0.0,
                        });
                    metrics.route_metrics.get_mut(&path).unwrap().hits += 1;
                    metrics.route_metrics.get_mut(&path).unwrap().avg_latency_ms = latency;

                    (StatusCode::from_u16(status.as_u16()).unwrap(), body).into_response()
                }
                Err(e) => {
                    let mut metrics = state.metrics.write().await;
                    metrics.total_errors += 1;
                    (StatusCode::BAD_GATEWAY, format!("Upstream error: {}", e)).into_response()
                }
            }
        }
        None => (StatusCode::NOT_FOUND, "No route configured").into_response(),
    }
}

async fn metrics_middleware(
    State(state): State<AppState>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let start = Instant::now();
    let method = req.method().clone();
    let uri = req.uri().clone();
    let response = next.run(req).await;
    let latency = start.elapsed().as_millis() as f64;

    let mut metrics = state.metrics.write().await;
    metrics.total_requests += 1;
    metrics.total_latency_ms += latency;
    if !response.status().is_success() {
        metrics.total_errors += 1;
    }

    tracing::info!(
        method = %method,
        uri = %uri,
        status = %response.status(),
        latency_ms = latency,
        "request completed"
    );

    response
}

// ─── Main ──────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env()
            .add_directive(tracing::Level::INFO.into()))
        .init();

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".into());
    let port: u16 = port.parse().expect("PORT must be a number");

    let route_table = Arc::new(RouteTable {
        routes: vec![
            Route {
                path: "/api/*".into(),
                methods: vec!["GET".into(), "POST".into()],
                upstreams: vec![Upstream {
                    id: "fleet-dashboard".into(),
                    url: "http://localhost:8889".into(),
                    weight: 1.0,
                    priority: 1,
                    tags: vec!["dashboard".into(), "fleet".into()],
                }],
                ternary_preference: TernaryRoute::Accept,
            },
        ],
    });

    let state = AppState {
        route_table,
        metrics: Arc::new(RwLock::new(ApiMetrics::default())),
        start_time: Instant::now(),
    };

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/metrics", get(get_metrics))
        .route("/api/routes", get(get_routes))
        .fallback(any(proxy_handler))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .layer(middleware::from_fn_with_state(state.clone(), metrics_middleware))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    tracing::info!("api-gateway listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
