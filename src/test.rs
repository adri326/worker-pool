use super::*;
use std::time::Duration;

#[test]
fn test_execute() {
    let mut pool: WorkerPool<usize, ()> = WorkerPool::new(16);

    pool.execute(|tx, _rx| {
        tx.send(1).unwrap();
    });

    std::thread::sleep(Duration::new(0, 100_000_000));

    let results = pool.recv_burst().collect::<Vec<_>>();
    assert_eq!(results, vec![1]);
}

#[test]
fn test_receive_burst() {
    let mut pool: WorkerPool<usize, ()> = WorkerPool::new(16);

    pool.execute(|tx, _rx| {
        tx.send(1).unwrap();
        std::thread::sleep(Duration::new(0, 200_000_000));
        tx.send(4).unwrap();
    });

    pool.execute(|tx, _rx| {
        tx.send(2).unwrap();
        std::thread::sleep(Duration::new(0, 200_000_000));
        tx.send(5).unwrap();
    });

    pool.execute(|tx, _rx| {
        tx.send(3).unwrap();
        std::thread::sleep(Duration::new(0, 200_000_000));
        tx.send(6).unwrap();
    });

    std::thread::sleep(Duration::new(0, 100_000_000));

    let mut results = pool.recv_burst().collect::<Vec<_>>();
    results.sort();
    assert_eq!(results, vec![1, 2, 3]);
}

#[test]
fn test_receive_all() {
    let mut pool: WorkerPool<usize, ()> = WorkerPool::new(16);

    pool.execute(|tx, _rx| {
        tx.send(1).unwrap();
        std::thread::sleep(Duration::new(0, 200_000_000));
        tx.send(4).unwrap();
    });

    pool.execute(|tx, _rx| {
        tx.send(2).unwrap();
        std::thread::sleep(Duration::new(0, 200_000_000));
        tx.send(5).unwrap();
    });

    pool.execute(|tx, _rx| {
        tx.send(3).unwrap();
        std::thread::sleep(Duration::new(0, 200_000_000));
        tx.send(6).unwrap();
    });

    let mut results = pool.stop().collect::<Vec<_>>();
    results.sort();
    assert_eq!(results, vec![1, 2, 3, 4, 5, 6]);
}

#[test]
fn test_execute_many() {
    let mut pool: WorkerPool<usize, ()> = WorkerPool::new(16);

    pool.execute_many(4, |tx, _rx| {
        tx.send(0).unwrap();
    });

    let results = pool.stop().collect::<Vec<_>>();
    assert_eq!(results, vec![0; 4]);
}

// Tests RecvAllIterator for deadlocks, when the bottleneck is both in the workers and in the organizer
#[test]
fn test_receive_all_bottleneck() {
    panic_after(Duration::new(1, 0), || {
        let mut pool: WorkerPool<usize, ()> = WorkerPool::new(1);

        pool.execute(|tx, _rx| {
            tx.send(1).unwrap();
            std::thread::sleep(Duration::new(0, 200_000_000));
            tx.send(4).unwrap();
        });

        pool.execute(|tx, _rx| {
            tx.send(2).unwrap();
            std::thread::sleep(Duration::new(0, 200_000_000));
            tx.send(5).unwrap();
        });

        pool.execute(|tx, _rx| {
            tx.send(3).unwrap();
            std::thread::sleep(Duration::new(0, 200_000_000));
            tx.send(6).unwrap();
        });

        let mut results = Vec::new();
        for x in pool.stop() {
            results.push(x);
            std::thread::sleep(Duration::new(0, 50_000_000));
        }
        results.sort();
        assert_eq!(results, vec![1, 2, 3, 4, 5, 6]);
    });
}

// Tests RecvAllIterator for deadlocks, when the main bottleneck is in the organizer
#[test]
fn test_receive_all_bottleneck2() {
    panic_after(Duration::new(2, 0), || {
        let mut pool: WorkerPool<usize, ()> = WorkerPool::new(1);

        pool.execute(|tx, _rx| {
            tx.send(1).unwrap();
            std::thread::sleep(Duration::new(0, 10_000_000));
            tx.send(4).unwrap();
        });

        pool.execute(|tx, _rx| {
            tx.send(2).unwrap();
            std::thread::sleep(Duration::new(0, 10_000_000));
            tx.send(5).unwrap();
        });

        pool.execute(|tx, _rx| {
            tx.send(3).unwrap();
            std::thread::sleep(Duration::new(0, 10_000_000));
            tx.send(6).unwrap();
        });

        let mut results = Vec::new();
        for x in pool.stop() {
            results.push(x);
            std::thread::sleep(Duration::new(0, 200_000_000));
        }
        results.sort();
        assert_eq!(results, vec![1, 2, 3, 4, 5, 6]);
    })
}

#[test]
#[should_panic]
fn test_join_panic() {
    let mut pool: WorkerPool<(), ()> = WorkerPool::new(1);

    pool.execute(|_tx, _rx| {
        panic!("Oh no I panicked");
    });

    pool.stop_and_join();
}

#[test]
#[should_panic]
fn test_stop_panic() {
    let mut pool: WorkerPool<(), ()> = WorkerPool::new(1);

    pool.execute(|_tx, _rx| {
        panic!("Oh no I panicked");
    });

    let _ = pool.stop().collect::<Vec<_>>();
}

#[test]
fn test_broadcast() {
    let mut pool: WorkerPool<usize, usize> = WorkerPool::new(2);

    pool.execute_many(2, |tx, rx| {
        loop {
            match rx.recv() {
                Ok(DownMsg::Other(x)) => tx.send(x).unwrap(),
                Ok(DownMsg::Stop) => break,
                _ => {}
            }
        }
        tx.send(10).unwrap();
    });

    pool.broadcast(DownMsg::Other(0));
    pool.broadcast(DownMsg::Other(1));
    std::thread::sleep(Duration::new(0, 100_000_000));
    pool.broadcast(DownMsg::Other(2));

    let mut results = pool.stop().collect::<Vec<_>>();
    results.sort();
    assert_eq!(results, vec![0, 0, 1, 1, 2, 2, 10, 10]);
}

// Test that recv_burst does not cause a livelock or deadlock
#[test]
fn test_burst() {
    panic_after(Duration::new(1, 0), || {
        let mut pool: WorkerPool<usize, ()> = WorkerPool::new(16);

        pool.execute_many(32, |tx, rx| {
            let mut i = 0;
            loop {
                match rx.try_recv() {
                    Ok(DownMsg::Stop) => break,
                    _ => {}
                }

                match tx.try_send(i) {
                    Ok(_) => i += 1,
                    Err(_) => {}
                }
            }
        });

        for _ in pool.recv_burst() {
            std::thread::sleep(Duration::new(0, 10_000_000));
        }

        for _ in pool.stop() {
            std::thread::sleep(Duration::new(0, 10_000_000));
        }
    });
}

#[test]
fn readme_deadlock() {
    panic_after(Duration::new(1, 0), || {
        // Here, `u64` is the type of the messages from the workers to the manager
        // `()` is the type of the messages from the manager to the workers
        // `32` is the maximum length of the message queue
        let mut pool: WorkerPool<u64, ()> = WorkerPool::new(32);

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

        // Sleep a bit
        std::thread::sleep(Duration::new(0, 100_000_000));

        // The iterator returned by pool.recv_burst() only yields messages received before it was created
        for x in pool.recv_burst() {
            println!("{}", x);
            std::thread::sleep(Duration::new(0, 10_000_000));
        }

        // pool.stop() also returns an iterator, which will send a Stop message and gather the worker's messages until they all finish
        for x in pool.stop() {
            println!("{}", x);
        }

        fn is_prime(n: u64) -> bool {
            for i in 2..n {
                if n % i == 0 {
                    return false
                }
            }
            true
        }
    });
}

fn panic_after<T, F>(d: Duration, f: F) -> T
where
    T: Send + 'static,
    F: FnOnce() -> T,
    F: Send + 'static,
{
    let (done_tx, done_rx) = channel();
    let handle = std::thread::spawn(move || {
        let val = f();
        done_tx.send(()).expect("Unable to send completion signal");
        val
    });

    match done_rx.recv_timeout(d) {
        Ok(_) => handle.join().expect("Thread panicked"),
        Err(_) => panic!("Thread took too long"),
    }
}
