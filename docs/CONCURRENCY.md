# TITAN Concurrency Runtime

TITAN tasks execute closures on real host threads. `spawn` returns a `Task` handle and `join` consumes that handle exactly once, propagating the task result or its structured VM error.

```titan
let task = spawn || expensive_work()
let result = join(task)
```

Bounded channels are created with `channel(capacity)` and return `(Sender, Receiver)`. Capacity zero provides rendezvous semantics. Values cross threads through Rust's memory-safe channels.

```titan
let endpoints = channel(16)
let tx = endpoints[0]
let rx = endpoints[1]

let producer = spawn || {
    send(tx, 42)
}

let value = recv(rx)
join(producer)
print(value)
```

Each task owns an isolated VM stack and locals while sharing an `Arc` runtime registry for task handles and channels. Closures capture values by value. Native capability settings and redirected output are inherited by child VMs. Task panics, duplicate joins, unknown handles, poisoned registries and disconnected channels become typed `VmError` values rather than host panics.

Cancellation is cooperative and checked before every bytecode instruction. `cancel(task)` sets an atomic token inherited by the child VM; `join(task)` then reports `TaskCancelled`. `join_timeout(task, milliseconds)` and `recv_timeout(receiver, milliseconds)` return `Option::Some` on completion/data and `Option::None` on timeout without consuming a pending task.

`select([receiverA, receiverB], timeout_ms)` waits across multiple receivers and returns `Option::Some((index, value))`, preserving which channel became ready. It uses bounded polling with a one-millisecond sleep rather than a busy loop, and reports disconnection explicitly.

Current semantics are blocking OS-thread concurrency. Structured task scopes and automatic child cleanup are the next layer; the implementation does not label these as async until async suspension semantics exist.

## Task Memory Quotas & Runtime Inspection (`std::runtime`)

To prevent rogue or unconstrained background tasks from exhausting process heap memory in enterprise server deployments, TITAN supports per-task memory quotas and runtime memory inspection:

- `std::runtime::spawn_quota(max_bytes, || { ... })` spawns a child task with an isolated VM whose heap allocation is strictly capped at `max_bytes`. If the child task exceeds this quota, it terminates cleanly with a typed `VmError::MemoryLimit`, which can be caught via `std::try::catch` without impacting other running tasks.
- `std::runtime::memory_limit()` returns the active memory quota for the current task (`-1` if unlimited).
- `std::runtime::allocated_bytes()` reports the cumulative bytes accounted against the current VM's memory quota. Every allocation path is charged (arrays, tuples, structs, enums, string concatenation, native-call results, collection-method outputs, range literals, and bytes), and this same counter enforces the memory limit.
- `std::runtime::gc_live_count()` returns an approximate live-object estimate derived from accounted allocation pressure (allocated bytes / 64). TITAN does not yet have a tracing garbage collector, so this is an allocation-pressure proxy rather than a precise live-object count.
- `std::runtime::gc_collect()` is currently an honest no-op that returns `0`: there is no garbage collector yet, so there is nothing to sweep. Heap memory is reclaimed automatically by Rust's ownership when values go out of scope; a tracing GC is planned future work. It deliberately does not reduce the allocation counter, since that would let a program defeat the memory limit.
- `std::runtime::gc_threshold()` reports the retained GC threshold value in bytes.
- `std::runtime::gc_set_threshold(bytes)` stores a GC threshold value for forward compatibility. Because no automatic collector exists yet, the threshold does not currently trigger any sweep; it is reserved for the future tracing GC.
- `std::runtime::active_tasks()` reports the number of concurrent tasks currently active in the VM process.
- `std::runtime::heap_dump(path)` exports an instantaneous JSON heap and diagnostic snapshot (`timestamp_unix_ms`, `allocated_bytes`, `memory_limit`, `gc_threshold`, `gc_live_count`, `active_tasks`, `status`) to the specified path without interrupting server execution.
- `std::runtime::optimize_level()` reports the active VM optimization level (`2` for release/fast-path mode).
- `std::runtime::fast_path_enabled()` returns `true` when inline integer arithmetic and comparison fast-paths are active.
- `std::runtime::benchmark(iterations, || { ... })` runs the closure in a tight execution loop for `iterations` passes, reporting a structured map containing `iterations`, `total_ms`, `ns_per_op`, and `ops_per_sec` for performance regression testing.

