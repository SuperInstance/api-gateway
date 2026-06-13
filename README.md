# API Gateway

An **API gateway** sits between clients and backend services, providing a single entry point for request routing, composition, and protocol translation.

## Why It Matters

Gateways centralize cross-cutting concerns: authentication, rate limiting, request shaping, response caching, observability. Without one, every service must reimplement these concerns. Essential for microservice architectures.

## How It Works

Implements reverse proxy routing with middleware pipeline: auth → rate limit → validate → route → transform → cache → log. Supports path-based routing, header-based routing, and request/response composition.

## Usage

```toml
[dependencies]
api-gateway = "0.1.0"
```

```rust
use api_gateway;

// See examples/ directory for detailed usage
```

## API

API documentation is generated from source doc-comments.

## Architecture

This crate is part of the **[SuperInstance](https://github.com/SuperInstance)** ecosystem — a conservation-law-based framework for fleet coordination, ternary computation, and distributed agent systems.

### Related Crates

- [`superinstance-core`](https://github.com/SuperInstance/superinstance-core) — Core conservation law (γ + η = C)
- [`superinstance-harness`](https://github.com/SuperInstance/superinstance-harness) — Build harness and self-improving loop
- [`fleet-coordinator`](https://github.com/SuperInstance/fleet-coordinator) — Fleet-level coordination

## References

- [SuperInstance Architecture](https://github.com/SuperInstance/SuperInstance/blob/main/ARCHITECTURE.md)
- [Conservation Law Paper](https://github.com/SuperInstance/SuperInstance/blob/main/docs/conservation-law.md)

## License

MIT
