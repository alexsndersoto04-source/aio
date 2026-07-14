# TITAN Server Lifecycle and Backpressure

`std::server::control(maximum_connections)` creates a shared lifecycle handle. Before spawning a connection task, call `try_acquire`; false means the server is draining or at capacity and should reject/close the accepted socket. Every acquired slot must call `release` when its connection task ends.

`shutdown(control)` atomically stops new acquisitions while existing tasks drain. `stats(control)` returns maximum, active, accepted, rejected, completed, ready, healthy and shutting_down. All state uses atomics; acquisition uses compare-exchange, so concurrent accept loops cannot exceed the configured maximum.

Readiness becomes false immediately at shutdown, while health remains true during draining. Active reaches zero when graceful shutdown is complete. Counters are monotonic and suitable for exporting through `std::metrics` or a health endpoint.
