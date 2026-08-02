# TITAN Metrics (`std::metrics`)

The process-wide metrics registry is thread-safe and intended for HTTP middleware, task runtimes and application observability in enterprise production environments.

## Operations

- `std::metrics::counter_add(name, amount)` uses saturating unsigned counters and returns the updated value.
- `std::metrics::counter_get(name)` retrieves the current value of a counter without modifying it (returns `0` if unset).
- `std::metrics::gauge_set(name, value)` stores finite floating-point gauges.
- `std::metrics::gauge_get(name)` retrieves the current floating-point gauge value without modifying it (returns `0.0` if unset).
- `std::metrics::histogram_record(name, value)` tracks count, sum, min and max without retaining samples.
- `std::metrics::snapshot()` returns consistent cloned maps suitable for JSON/health endpoints.
- `std::metrics::prometheus_export()` returns a string formatted in the standard Prometheus 0.0.4 / OpenMetrics text exposition format (`# TYPE <name> counter/gauge/summary`), ready for scraping by Prometheus, Grafana or Datadog agents without external dependencies.
- `std::metrics::reset()` clears the registry for tests or controlled lifecycle resets.

Metric names are limited to 200 ASCII alphanumeric, dot, underscore or hyphen characters. NaN/infinite values and invalid names are rejected. All registry access uses `RwLock`; poisoning becomes a structured error rather than a panic.
