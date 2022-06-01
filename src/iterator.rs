use super::*;
use std::time::Duration;

pub struct RecvBurstIterator<'a, Up: Send + 'static> {
    receiver: &'a Receiver<UpMsg<Up>>,
    buffer_prev: &'a mut Option<UpMsg<Up>>,
    start: Instant
}

impl<'a, Up: Send + 'static> RecvBurstIterator<'a, Up> {
    #[inline]
    pub fn new(
        receiver: &'a Receiver<UpMsg<Up>>,
        buffer_prev: &'a mut Option<UpMsg<Up>>,
        start: Instant
    ) -> Self {
        Self {
            receiver,
            buffer_prev,
            start
        }
    }
}

// This iterator *should* be fused, although the following scenario is still possible
// Worker sends a message
// RecvBurstIterator is created
// RecvBurstIterator::next() is called ~> yields None
// The message is received ~> RecvBurstIterator::next() will now yield Some
impl<'a, Up: Send + 'static> Iterator for RecvBurstIterator<'a, Up> {
    type Item = Up;

    #[inline]
    fn next(&mut self) -> Option<Up> {
        if let Some(ref prev) = self.buffer_prev {
            if prev.time() > self.start {
                return None
            } else {
                // std::mem::drop(prev); // Drop the immutable reference

                return std::mem::replace(self.buffer_prev, None).map(|m| m.get());
            }
        }

        match self.receiver.try_recv() {
            Ok(msg) => {
                if msg.time() > self.start {
                    *self.buffer_prev = Some(msg);
                    None
                } else {
                    Some(msg.get())
                }
            }
            Err(_) => None
        }
    }
}

pub struct RecvAllIterator<Up: Send + 'static> {
    receiver: Receiver<UpMsg<Up>>,
    ready_receiver: Option<Receiver<()>>, // If Some, then not all workers are in the ready state
    buffer_prev: Option<UpMsg<Up>>,
    workers: Vec<JoinHandle<Option<Up>>>,

    ready_count: usize,
}

impl<Up: Send + 'static> RecvAllIterator<Up> {
    pub fn new(
        receiver: Receiver<UpMsg<Up>>,
        ready_receiver: Receiver<()>,
        buffer_prev: Option<UpMsg<Up>>,
        workers: Vec<JoinHandle<Option<Up>>>,
    ) -> Self {
        Self {
            receiver: receiver,
            ready_receiver: Some(ready_receiver),
            buffer_prev,
            workers,
            ready_count: 0
        }
    }

    #[inline]
    fn update_ready_count(&mut self) {
        if let Some(ready_receiver) = &self.ready_receiver {
            while let Ok(_) = ready_receiver.try_recv() {
                self.ready_count += 1;
            }

            if self.ready_count == self.workers.len() {
                self.ready_receiver = None;
            }
        }
    }
}

const READY_DELAY: Duration = Duration::new(0, 1_000_000);

impl<Up: Send + 'static> Iterator for RecvAllIterator<Up> {
    type Item = Up;

    #[inline]
    fn next(&mut self) -> Option<Up> {
        self.update_ready_count();

        // First, return buffer_prev
        if self.buffer_prev.is_some() {
            let buffer_prev = std::mem::replace(&mut self.buffer_prev, None).unwrap();
            return Some(buffer_prev.get());
        }

        // Then, return any message in receiver
        if let Ok(msg) = self.receiver.try_recv() {
            return Some(msg.get());
        }

        while let Some(ready_receiver) = &self.ready_receiver {
            if let Ok(_) = ready_receiver.recv_timeout(READY_DELAY) {
                self.ready_count += 1;

                if self.ready_count == self.workers.len() {
                    self.ready_receiver = None;
                }
            }

            if let Ok(msg) = self.receiver.try_recv() {
                return Some(msg.get());
            }
        }

        debug_assert!(self.ready_receiver.is_none());

        // ready_receiver is None, so we can safely join the threads; `receiver` will now always yield Err

        if self.workers.len() == 0 {
            return None
        }

        while let Some(worker) = self.workers.pop() {
            match worker.join() {
                Ok(Some(msg)) => return Some(msg),
                Ok(None) => {}
                Err(e) => std::panic::resume_unwind(e),
            }
        }

        debug_assert!(self.workers.len() == 0);

        None
    }
}

impl<Up: Send + 'static> std::iter::FusedIterator for RecvAllIterator<Up> {}
