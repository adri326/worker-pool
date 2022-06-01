use super::*;

// Here for future-proofing, in case we want to switch over to another mpsc implementation
pub type Sender<T> = std::sync::mpsc::Sender<T>;
pub type SyncSender<T> = std::sync::mpsc::SyncSender<T>;
pub type Receiver<T> = std::sync::mpsc::Receiver<T>;
pub type TryRecvError = std::sync::mpsc::TryRecvError;
pub type RecvError = std::sync::mpsc::RecvError;
pub type SendError<T> = std::sync::mpsc::SendError<T>;
pub type TrySendError<T> = std::sync::mpsc::TrySendError<T>;

/// A message sent from the organizer to the workers
pub enum DownMsg<Down: Send = ()> {
    Stop,
    Pause,
    Continue,
    Other(Down)
}

/// A message sent from a worker to the organizer; contains the timestamp of its creation to
/// allow RecvBurstIterator to stop early
pub struct UpMsg<Up: Send = ()> {
    time: Instant,
    msg: Up
}

/// A wrapper around Sender<UpMsg<Up>>
#[non_exhaustive]
pub struct WorkerSender<Up: Send = ()> {
    pub sender: SyncSender<UpMsg<Up>>,
}

impl<Up: Send> From<SyncSender<UpMsg<Up>>> for WorkerSender<Up> {
    fn from(sender: SyncSender<UpMsg<Up>>) -> Self {
        Self {
            sender
        }
    }
}

/// A wrapper around Receiver<DownMsg<Down>>
pub type WorkerReceiver<Down> = Receiver<DownMsg<Down>>;

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
    pub fn send(&self, msg: Up) -> Result<(), SendError<Up>> {
        self.sender.send(UpMsg::new(msg)).map_err(|e| std::sync::mpsc::SendError(e.0.get()))
    }

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
