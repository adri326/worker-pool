use super::*;

// Here for future-proofing, in case we want to switch over to another mpsc implementation
pub type Sender<T> = std::sync::mpsc::Sender<T>;
pub type SyncSender<T> = std::sync::mpsc::SyncSender<T>;
pub type Receiver<T> = std::sync::mpsc::Receiver<T>;
pub type TryRecvError = std::sync::mpsc::TryRecvError;
pub type RecvError = std::sync::mpsc::RecvError;
pub type SendError<T> = std::sync::mpsc::SendError<T>;
pub type TrySendError<T> = std::sync::mpsc::TrySendError<T>;

// TODO: add unsafe ways to manually send messages

/// A message sent from the manager to the workers
#[derive(Clone, Debug, PartialEq)]
pub enum DownMsg<Down: Send> {
    /// Instructs the workers to stop execution and return as soon as possible.
    /// See [`WorkerPool::execute`] on more information as to how this value should be handled.
    Stop,
    /// Instructs a worker to pause execution until the `Continue` message is received.
    /// This value can safely be ignored, and what to do when another message is received while in the paused state is up to the worker.
    Pause,
    /// Instructs a worker to resume execution.
    /// This value can safely be ignored.
    Continue,
    /// A customized message sent from the manager to the workers.
    /// The `Down` type must implement [`Send`] and optionally [`Clone`] so that the messages can be sent.
    Other(Down)
}

/// A message sent from a worker to the manager; contains the timestamp of its creation to
/// allow [`RecvBurstIterator`] to stop early.
#[derive(Clone, Debug)]
pub struct UpMsg<Up: Send> {
    time: Instant,
    msg: Up
}

/// A wrapper around `Sender<UpMsg<Up>>`. This type implements `!Send`,
/// as [`RecvAllIterator`] depends on this type being dropped whenever the thread holding it stops or panics.
#[derive(Clone, Debug)]
pub struct WorkerSender<Up: Send> {
    sender: SyncSender<UpMsg<Up>>,
}

impl<Up: Send> UpMsg<Up> {
    pub fn new(msg: Up) -> Self {
        Self {
            time: Instant::now(),
            msg
        }
    }

    pub fn time(&self) -> Instant {
        self.time
    }

    pub fn get(self) -> Up {
        self.msg
    }
}

impl<Up: Send> WorkerSender<Up> {
    pub(crate) fn new(sender: SyncSender<UpMsg<Up>>) -> Self {
        Self {
            sender
        }
    }

    /// Sends a value on the channel, blocking if the channel is full.
    /// The value is wrapped in an [`UpMsg`]
    pub fn send(&self, msg: Up) -> Result<(), SendError<Up>> {
        self.sender.send(UpMsg::new(msg)).map_err(|e| std::sync::mpsc::SendError(e.0.get()))
    }

    /// Tries to send a value on the channel. If the channel is full, then [`TrySendError::Full`] is returned instead.
    pub fn try_send(&self, msg: Up) -> Result<(), TrySendError<Up>> {
        use std::sync::mpsc::TrySendError;

        self.sender.try_send(UpMsg::new(msg)).map_err(|e| {
            match e {
                TrySendError::Full(x) => TrySendError::Full(x.get()),
                TrySendError::Disconnected(x) => TrySendError::Disconnected(x.get())
            }
        })
    }
}

impl<Up: Send> !Send for WorkerSender<Up> {}

/// A wrapper around Receiver<DownMsg<Down>>
pub type WorkerReceiver<Down> = Receiver<DownMsg<Down>>;
