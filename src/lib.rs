#![feature(duration_consts_2)]

use std::thread::{JoinHandle};
use std::marker::PhantomData;
use std::time::Instant;

use std::sync::mpsc::{channel, sync_channel};

// TODO: support for robust causality as a feature

#[cfg(test)]
mod test;

mod iterator;
pub use iterator::*;

mod msg;
pub use msg::*;

pub struct WorkerPool<Up, Down = ()>
where
    Up: Send + 'static,
    Down: Clone + Send + 'static,
{
    channel: (SyncSender<UpMsg<Up>>, Receiver<UpMsg<Up>>),
    ready_channel: (Sender<()>, Receiver<()>),
    buffer_length: usize,
    buffer_prev: Option<UpMsg<Up>>,

    workers: Vec<(JoinHandle<Option<Up>>, Sender<DownMsg<Down>>)>,

    phantoms: PhantomData<(Up, Down)>
}

impl<Up, Down> WorkerPool<Up, Down>
where
    Up: Send + 'static,
    Down: Clone + Send + 'static,
{
    #[inline]
    pub fn new(buffer_length: usize) -> Self {
        Self {
            channel: sync_channel(buffer_length),
            ready_channel: channel(),
            buffer_length,
            buffer_prev: None,
            workers: Vec::new(),
            phantoms: PhantomData,
        }
    }

    #[inline]
    pub fn execute<F>(&mut self, callback: F)
    where
        F: (FnOnce(WorkerSender<Up>, Receiver<DownMsg<Down>>) -> Option<Up>),
        F: Send + 'static
    {
        let (down_tx, down_rx) = channel();
        let up_tx = self.channel.0.clone();
        let ready_tx = self.ready_channel.0.clone();
        self.workers.push((
            std::thread::spawn(move || {
                let res = (callback)(WorkerSender::from(up_tx), down_rx);

                // Will only fail if RecvAllIterator is dropped, in which case we don't care about the result
                let _ = ready_tx.send(());
                res
            }),
            down_tx
        ));
    }

    #[inline]
    pub fn execute_many<F>(&mut self, n_workers: usize, callback: F)
    where
        F: (FnOnce(WorkerSender<Up>, Receiver<DownMsg<Down>>) -> Option<Up>),
        F: Clone + Send + 'static
    {
        for _n in 0..n_workers {
            self.execute(callback.clone());
        }
    }

    #[inline]
    pub fn buffer_length(&self) -> usize {
        self.buffer_length
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
    /// allowing you to execute more tasks immediately
    pub fn stop(&mut self) -> RecvAllIterator<Up> {
        let ready_channel = std::mem::replace(&mut self.ready_channel, channel());
        let channel = std::mem::replace(&mut self.channel, sync_channel(self.buffer_length));
        let buffer_prev = std::mem::replace(&mut self.buffer_prev, None);
        let workers_len = self.workers.len();
        let workers = std::mem::replace(&mut self.workers, Vec::with_capacity(workers_len));

        let workers = workers.into_iter().map(|w| {
            // Note: the only instance where this can fail is if the receiver was dropped,
            // in which case we can only hope that the thread will eventually join
            let _ = w.1.send(DownMsg::Stop);
            w.0
        }).collect::<Vec<_>>();

        RecvAllIterator::new(
            channel.1,
            ready_channel.1,
            buffer_prev,
            workers
        )
    }

    // fn recv(&mut self) -> Result<UpMsg<Up>, RecvError> {
    //     if self.buffer_prev.is_some() {
    //         Ok(std::mem::replace(&mut self.buffer_prev, None).unwrap())
    //     } else {
    //         self.channel.1.recv()
    //     }
    // }

    // fn try_recv(&mut self) -> Result<UpMsg<Up>, TryRecvError> {
    //     if self.buffer_prev.is_some() {
    //         Ok(std::mem::replace(&mut self.buffer_prev, None).unwrap())
    //     } else {
    //         self.channel.1.try_recv()
    //     }
    // }
}
