# TITAN Metrics

The process-wide metrics registry is thread-safe and intended for HTTP middleware, task runtimes and application observability.

- `std::metrics::counter_add(name, amount)` uses saturating unsigned counters.
- `gauge_set(name, value)` stores finite floating-point gauges.
- `histogram_record(name, value)` tracks count, sum, min and max without retaining samples.
- `snapshot()` returns consistent cloned maps suitable for JSON/health endpoints.
- `reset()` clears the registry for tests or controlled lifecycle resets.

Metric names are limited to 200 ASCII alphanumeric, dot, underscore or hyphen characters. NaN/infinite values and invalid names are rejected. All registry access uses `RwLock`; poisoning becomes a structured error rather than a panic.
