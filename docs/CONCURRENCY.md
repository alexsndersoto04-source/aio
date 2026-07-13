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
