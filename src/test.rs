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

// Tests RecvAllIterator for deadlocks, when the bottleneck is both in the workers and in the organizer
#[test]
fn test_receive_all_bottleneck() {
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
}

// Tests RecvAllIterator for deadlocks, when the main bottleneck is in the organizer
#[test]
fn test_receive_all_bottleneck2() {
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
