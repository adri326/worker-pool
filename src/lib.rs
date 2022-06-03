#![feature(negative_impls)]

use std::thread::{JoinHandle};
use std::marker::PhantomData;
use std::time::Instant;

use std::sync::mpsc::{channel, sync_channel};

// TODO: support for robust causality as a feature
// TODO: regular Sender

#[cfg(test)]
mod test;

mod iterator;
pub use iterator::*;

mod msg;
pub use msg::*;

mod crate_macros;

// As per this test: https://github.com/rust-lang/rust/blob/1.3.0/src/libstd/sync/mpsc/mod.rs#L1581-L1592
// it looks like std::sync::mpsc channels are meant to preserve the order sent within a single thread

/**
The main struct, represents a pool of worker.
The owner of this struct is the "Manager", while the threads handled by this struct are the "Workers".

# Example

```
use worker_pool::WorkerPool;

let mut pool: WorkerPool<String, ()> = WorkerPool::new(100);

pool.execute(|tx, _rx| {
    tx.send(String::from("Hello"));
    tx.send(String::from("world!"));
});

assert_eq!(pool.stop().collect::<Vec<_>>().join(" "), "Hello world!");
```
*/
pub struct WorkerPool<Up, Down>
where
    Up: Send + 'static,
    Down: Send + 'static,
{
    channel: (SyncSender<UpMsg<Up>>, Receiver<UpMsg<Up>>),
    buffer_length: usize,
    buffer_prev: Option<UpMsg<Up>>,

    workers: Vec<(JoinHandle<()>, Sender<DownMsg<Down>>)>,
    worker_index: usize,

    phantoms: PhantomData<(Up, Down)>
}

impl<Up, Down> WorkerPool<Up, Down>
where
    Up: Send + 'static,
    Down: Send + 'static,
{
    /**
    Creates a new WorkerPool instance, with a given maximum buffer length.

    The higher the buffer length, the higher the message throughput,
    but the higher the memory cost.
    See [SyncSender](https://doc.rust-lang.org/std/sync/mpsc/struct.SyncSender.html) for more information.

    # Example

    ```
    use std::time::Duration;
    use worker_pool::{WorkerPool, DownMsg};

    let mut pool: WorkerPool<usize, String> = WorkerPool::new(3);

    pool.execute(|tx, rx| {
        loop {
            let msg = worker_pool::recv_break!(rx);
            tx.send(msg.len()).unwrap();
        }
    });

    pool.broadcast(DownMsg::Other(String::from("Betelgeuse")));
    pool.broadcast(DownMsg::Other(String::from("Alpha Centauri")));
    pool.broadcast(DownMsg::Other(String::from("Sirius")));

    // When the worker will send the result for this message, tx.send will block, but the message
    // will still count as being sent before recv_burst(). Whether or not it will appear in the
    // iterator depends on the speed at which the items in the iterator are read.
    pool.broadcast(DownMsg::Other(String::from("Procyon")));

    pool.broadcast(DownMsg::Other(String::from("Sun")));

    std::thread::sleep(Duration::new(0, 100_000_000));

    assert_eq!(
        pool.recv_burst().take(3).collect::<Vec<_>>(),
        vec![10, 14, 6]
    );
    ```
    */
    #[inline]
    pub fn new(buffer_length: usize) -> Self {
        Self {
            channel: sync_channel(buffer_length),
            buffer_length,
            buffer_prev: None,
            workers: Vec::new(),
            worker_index: 0,
            phantoms: PhantomData,
        }
    }

    /**
    Spawns one worker thread with the given callback:

    ```
    # use worker_pool::*;
    # let mut pool: WorkerPool<(), ()> = WorkerPool::new(100);
    // Spawns 1 worker thread
    pool.execute(|tx, rx| {
        // Send messages in tx
        // Receive messages in rx
    });
    # pool.stop_and_join();
    ```

    To prevent any deadlocks, the worker thread *must* stop shortly after receiving the `Stop` message:
    - if it is in an infinite loop, then that loop must be broken (`recv_break!` and `try_recv_break!` will handle that for you)
    - after the `Stop` message is received, it may only wait for space in the buffer of `tx`
    - make sure that no lock will prevent a `Stop` message from being received
    - the worker thread may `panic!`, in which case its exception will be propagated up by `stop()` and `stop_and_join()`
    */
    #[inline]
    pub fn execute<F>(&mut self, callback: F)
    where
        F: (FnOnce(WorkerSender<Up>, Receiver<DownMsg<Down>>)),
        F: Send + 'static
    {
        let (down_tx, down_rx) = channel();
        let up_tx = self.channel.0.clone();
        self.workers.push((
            std::thread::spawn(move || {
                (callback)(WorkerSender::from(up_tx), down_rx);
            }),
            down_tx
        ));
    }

    /**
    Spawns `n` worker threads with the given callback.
    The callback must implement `Clone`.

    ```
    # use worker_pool::*;
    # let mut pool: WorkerPool<(), ()> = WorkerPool::new(100);
    // Spawns 16 worker thread
    pool.execute_many(16, |tx, rx| {
        // Send messages in tx
        // Receive messages in rx
    });
    # pool.stop_and_join();
    ```

    To prevent any deadlocks, the worker threads *must* stop shortly after receiving the `Stop` message.
    See [`execute`](#execute) for more information.
    */
    #[inline]
    pub fn execute_many<F>(&mut self, n_workers: usize, callback: F)
    where
        F: (FnOnce(WorkerSender<Up>, Receiver<DownMsg<Down>>)),
        F: Clone + Send + 'static
    {
        for _n in 0..n_workers {
            self.execute(callback.clone());
        }
    }

    /// Returns the maximum length of the message queue
    #[inline]
    pub fn buffer_length(&self) -> usize {
        self.buffer_length
    }

    /// Receives a single message from a worker; blocking.
    /// If you need to call this function repeatedly, then consider iterating over the result of `recv_burst` instead.
    pub fn recv(&mut self) -> Result<Up, RecvError> {
        if self.buffer_prev.is_some() {
            return Ok(std::mem::replace(&mut self.buffer_prev, None).unwrap().get());
        }

        self.channel.1.recv().map(|x| x.get())
    }

    /// Returns an iterator that will yield a "burst" of messages.
    /// This iterator will respect causality, meaning that it will not yield any message that were sent after it was created.
    /// You can thus safely iterate over all of the elements of this iterator without risking a livelock.
    pub fn recv_burst<'b>(&'b mut self) -> RecvBurstIterator<'b, Up> {
        let start = Instant::now();

        RecvBurstIterator::new(
            &self.channel.1,
            &mut self.buffer_prev,
            start
        )
    }

    /// Stops the execution of all threads, returning an iterator that will yield and join all of the
    /// messages from the workers. As soon as this function returns, the WorkerPool will be back to its starting state,
    /// allowing you to execute more tasks immediately.
    ///
    /// The returned iterator will read all of the remaining messages one by one.
    /// Once the last message is received, it will join all threads.
    pub fn stop(&mut self) -> RecvAllIterator<Up> {
        let channel = std::mem::replace(&mut self.channel, sync_channel(self.buffer_length));
        let buffer_prev = std::mem::replace(&mut self.buffer_prev, None);
        let workers_len = self.workers.len();
        let workers = std::mem::replace(&mut self.workers, Vec::with_capacity(workers_len));
        self.worker_index = 0;

        let workers = workers.into_iter().map(|worker| {
            // Note: the only instance where this can fail is if the receiver was dropped,
            // in which case we can only hope that the thread will eventually join
            let _ = worker.1.send(DownMsg::Stop);
            worker.0
        }).collect::<Vec<_>>();

        RecvAllIterator::new(
            channel.1,
            buffer_prev,
            workers
        )
    }

    /// Stops the execution of all threads and joins them. Returns a Vec containing all of the remaining yielded values.
    /// Note that the returned Vec will ignore the `buffer_length` limitation.
    #[inline]
    pub fn stop_and_join(&mut self) -> Vec<Up> {
        let (sender, receiver) = std::mem::replace(&mut self.channel, sync_channel(self.buffer_length));
        std::mem::drop(sender); // Prevent deadlock
        let buffer_prev = std::mem::replace(&mut self.buffer_prev, None);
        let workers_len = self.workers.len();
        let workers = std::mem::replace(&mut self.workers, Vec::with_capacity(workers_len));
        self.worker_index = 0;

        for worker in workers.iter() {
            // Note: the only instance where this can fail is if the receiver was dropped,
            // in which case we can only hope that the thread will eventually join
            let _ = worker.1.send(DownMsg::Stop);
        }

        let mut res = Vec::new();

        if let Some(buffer_prev) = buffer_prev {
            res.push(buffer_prev.get());
        }

        while let Ok(msg) = receiver.recv() {
            res.push(msg.get());
        }

        for worker in workers {
            match worker.0.join() {
                Ok(_) => {},
                Err(e) => std::panic::resume_unwind(e),
            }
        }

        res
    }

    /// Sends `msg` to every worker
    pub fn broadcast(&self, msg: DownMsg<Down>) where Down: Clone {
        for (_join, tx) in self.workers.iter() {
            // This will fail iff the thread has dropped its receiver, in which case we
            // don't want for it to affect the other threads
            let _ = tx.send(msg.clone());
        }
    }

    /// Sends `msg` to a single worker, in a round-robin fashion.
    /// Returns Err if there is no worker or if the worker has dropped its receiver.
    pub fn broadcast_one(&mut self, msg: DownMsg<Down>) -> Result<(), SendError<DownMsg<Down>>> {
        if self.workers.len() == 0 {
            return Err(std::sync::mpsc::SendError(msg))
        }

        self.worker_index = (self.worker_index + 1) % self.workers.len();
        self.workers[self.worker_index].1.send(msg)
    }

    pub fn get(&self, index: usize) -> Option<(&JoinHandle<()>, Sender<DownMsg<Down>>)> {
        match self.workers.get(index) {
            Some(x) => Some((&x.0, x.1.clone())),
            None => None
        }
    }
}
