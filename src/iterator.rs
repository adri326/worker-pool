use super::*;

/// An iterator that will yield received messages until the message queue has been caught up to when the iterator was created.
pub struct RecvBurstIterator<'a, Up: Send + 'static> {
    receiver: &'a Receiver<UpMsg<Up>>,
    buffer_prev: &'a mut Option<UpMsg<Up>>,
    start: Instant
}

impl<'a, Up: Send + 'static> RecvBurstIterator<'a, Up> {
    #[inline]
    pub(crate) fn new(
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

/// An iterator that will yield all the remaining messages from the workers, and join them once they have all dropped their receiver.
pub struct RecvAllIterator<Up: Send + 'static> {
    receiver: Receiver<UpMsg<Up>>,
    buffer_prev: Option<UpMsg<Up>>,
    workers: Vec<JoinHandle<()>>,
}

impl<Up: Send + 'static> RecvAllIterator<Up> {
    pub(crate) fn new(
        receiver: Receiver<UpMsg<Up>>,
        buffer_prev: Option<UpMsg<Up>>,
        workers: Vec<JoinHandle<()>>,
    ) -> Self {
        Self {
            receiver: receiver,
            buffer_prev,
            workers,
        }
    }
}

impl<Up: Send + 'static> Iterator for RecvAllIterator<Up> {
    type Item = Up;

    #[inline]
    fn next(&mut self) -> Option<Up> {
        if self.buffer_prev.is_some() {
            let buffer_prev = std::mem::replace(&mut self.buffer_prev, None);
            return Some(buffer_prev.unwrap().get());
        }

        if let Ok(msg) = self.receiver.recv() {
            Some(msg.get())
        } else {
            for worker in std::mem::replace(&mut self.workers, Vec::new()) {
                match worker.join() {
                    Ok(_) => {}
                    Err(e) => std::panic::resume_unwind(e)
                }
            }

            None
        }
    }
}

impl<Up: Send + 'static> std::iter::FusedIterator for RecvAllIterator<Up> {}
