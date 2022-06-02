# worker-pool

A rust crate to handle a set of worker threads, which need to communicate back their result to the main thread.

This crate implements a specialization of the actor model, with the following properties:
- there is one manager and many workers
- workers may work indefinitely
- workers can send messages up to the manager; these messages must not be lost
- the manager can broadcast messages to the workers
- the manager can ask the workers to stop their work at any time, in which case the system should reach a ready state as fast as possible
- no deadlocks¹ and no livelocks

¹: unless specified in the documentation and under the assumption that every worker handles the `Stop` message and stops some time after

## Installation and usage

Add this library to your `Cargo.toml`:

```toml
worker-pool = "0.1.0"
```

Then, create a new instance of `worker_pool::WorkerPool`:

```rust
use std::time::Duration;
use worker_pool::{WorkerPool, DownMsg};

// Here, `u64` is the type of the messages from the workers to the manager
// `()` is the type of the messages from the manager to the workers
// `32` is the maximum length of the message queue
let pool: WorkerPool<u64, ()> = WorkerPool::new(32);
```

To start a new thread, use `WorkerPool::execute` or `WorkerPool::execute_many`:

```rust
fn is_prime(n: u64) -> bool {
    for i in 2..n {
        if n % i == 0 {
            return false
        }
    }
    true
}

// Start a new thread, that will communicate with the manager
pool.execute(|tx, rx| {
    tx.send(2).unwrap();
    let mut n = 3;
    loop {
        match rx.try_recv() {
            Ok(DownMsg::Stop) => break,
            _ => {}
        }

        while !is_prime(n) {
            n += 2;
        }

        tx.send(n).unwrap(); // The program may block here; this will not cause a deadlock
    }
});
```

Then, you can interact with the threads using the different methods of `WorkerPool`, notably `recv_burst`, `broadcast` and `stop`:

```rust
// Give the worker(s) time to boot up
std::thread::sleep(Duration::new(0, 100_000_000));

// The iterator returned by pool.recv_burst() only yields messages received before it was created and is non-blocking,
// so no livelock or deadlock can occur when using it.
for x in pool.recv_burst() {
    println!("{}", x);
    std::thread::sleep(Duration::new(0, 10_000_000));
}

// `pool.stop()` also returns an iterator, which will send a Stop message
// and gather the worker's messages until they all finish. No deadlock can occur here,
// as long as the threads' only shared, critical resource are the worker/manager channels
// provided by WorkerPool and the threads eventually respond to the `Stop` message.
for x in pool.stop() {
    println!("{}", x);
}
```

## License

This project is dual-licensed under the MIT license and the Apache v2.0 license.
You may choose either of those when using this library.

Any contribution to this repository must be made available under both licenses.
