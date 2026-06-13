# API Gateway

**API Gateway** is a Rust library providing the routing, request dispatch, and upstream management layer for the SuperInstance fleet's HTTP API infrastructure, directing incoming requests to appropriate backend services.

## Why It Matters

An API gateway is the single entry point that sits between clients and backend services. It handles cross-cutting concerns — request routing, authentication, rate limiting, load balancing, and response transformation — so that individual backend services can focus on business logic. In microservice architectures, the gateway pattern reduces client-side complexity (one URL instead of dozens), centralizes security policy enforcement, and provides a natural point for observability instrumentation. Without a gateway, every backend service must independently implement authentication, CORS, logging, and rate limiting — duplicating infrastructure code and creating inconsistent security postures across the fleet.

## How It Works

The gateway implements a **reverse proxy routing model**. Each incoming request is matched against a routing table:

```
route(request):
  for each (path_prefix, upstream) in routes:
    if request.path.starts_with(path_prefix):
      return forward(request, upstream)
  return 404
```

**Routing complexity:** O(R) for R routes with linear scan, or O(log R) with a trie-based router. For the SuperInstance fleet with ~20 services, linear scan at O(20) is negligible.

**Request lifecycle:**
1. **Ingress:** Receive HTTP request, parse method/path/headers
2. **Authentication:** Validate JWT or API key against `fleet-auth` D1 database
3. **Rate limiting:** Token bucket per client IP (capacity 100, refill 10/min)
4. **Routing:** Match path prefix to upstream service URL
5. **Forward:** Proxy request with timeout (30s default)
6. **Response:** Return upstream response, log metrics to `fleet-metrics-cron`

**Load balancing:** When multiple upstream instances are available, the gateway uses weighted round-robin, distributing load proportional to declared capacity. Health checks (HTTP GET `/health` every 10s) remove failed instances from rotation automatically.

## Quick Start

```rust
fn main() {
    println!("api-gateway: routing requests to upstreams");
    // The gateway runs as a Worker on Cloudflare's edge network,
    // routing to fleet services:
    //   /search    → fleet-vector-api
    //   /auth      → fleet-auth
    //   /metrics   → fleet-metrics-cron
    //   /ingest    → fleet-edge-worker
}
```

## API

| Endpoint | Upstream | Purpose |
|----------|----------|---------|
| `POST /search` | fleet-vector-api | Semantic crate search |
| `POST /auth/*` | fleet-auth | Authentication/authorization |
| `GET /metrics` | fleet-metrics-cron | Fleet performance metrics |
| `POST /ingest` | fleet-edge-worker | Bulk data ingestion |
| `GET /health` | self | Health check |

## Architecture Notes

The API Gateway is the **η-layer ingress point** in the SuperInstance fleet. It routes incoming intelligence requests (η) to the appropriate γ-layer computation services, enforcing the γ + η = C conservation contract: every request through the gateway is tracked, and resource consumption must balance against declared capacity limits.

See [ARCHITECTURE.md](https://github.com/SuperInstance/SuperInstance/blob/main/ARCHITECTURE.md).

**Rate limiting algorithm:** The gateway uses a token bucket per client IP. The bucket has capacity C = 100 tokens and refills at rate r = 10 tokens/minute. Each request consumes 1 token. If the bucket is empty, the request is rejected with HTTP 429 (Too Many Requests). This algorithm allows burst traffic (up to C requests instantly) while maintaining a steady-state rate of r requests/minute. The token bucket is O(1) per request: one timestamp comparison and one integer decrement.

**Observability:** Every request logs method, path, status, latency, and upstream to the fleet metrics pipeline. The p50/p95/p99 latency percentiles are tracked via a fixed-size circular histogram, enabling real-time SLO monitoring without storing individual request logs.

## References

1. Newman, S. (2021). *Building Microservices*. 2nd ed. O'Reilly. Chapter 8: The API Gateway Pattern.
2. Fielding, R.T. (2000). *Architectural Styles and the Design of Network-Based Software Architectures*. PhD Thesis, UC Irvine. Chapter 5: REST.

## License

MIT
