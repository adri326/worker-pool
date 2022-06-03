/**
Wrapper around `Receiver<DownMsg<T>>::recv`, meant to be used in a loop:

- if `Stop` or `Err` is received, breaks from the parent loop
- if `Pause` or `Continue` are received, do nothing
- if `Other(x)` is received, returns `x`

# Example

```
# use worker_pool::*;
# let mut pool: WorkerPool<(), usize> = WorkerPool::new(100);
pool.execute(|_tx, rx| {
    loop {
        let msg = worker_pool::recv_break!(rx);
        // Do something with msg
    }
});
# pool.broadcast(DownMsg::Other(10));
# pool.stop_and_join();
```
*/
#[macro_export]
macro_rules! recv_break {
    ( $rx:tt ) => {{
        let res = loop {
            match $rx.recv() {
                Ok(DownMsg::Stop) => break None,
                Ok(DownMsg::Pause) => {}
                Ok(DownMsg::Continue) => {},
                Ok(DownMsg::Other(x)) => break Some(x),
                Err(_) => break None
            }
        };

        match res {
            Some(x) => x,
            None => break
        }
    }}
}
/**
Wrapper around `Receiver<DownMsg<T>>::try_recv`, meant to be used in a loop:

- if `Stop` or `Disconnected` is received, breaks from the parent loop
- if `Pause` is received, then block until `Continue` is received; any `Other` message will be ignored between the two
- if `Continue` or `Empty` is received, returns `None`
- if `Other(x)` is received, returns `Some(x)`

# Example

```
# use worker_pool::*;
# let mut pool: WorkerPool<(), usize> = WorkerPool::new(100);
pool.execute(|_tx, rx| {
    # let mut count = 0;
    loop {
        if let Some(msg) = worker_pool::try_recv_break!(rx) {
            // Handle msg
            # count = msg;
        } else {
            // Do something else in the meantime
            # count += 1;
        }
    }
});
# pool.broadcast(DownMsg::Other(10));
# pool.stop_and_join();
```
*/
#[macro_export]
macro_rules! try_recv_break {
    ( $rx:tt ) => {{
        match $rx.try_recv() {
            Ok(DownMsg::Stop) => break,
            Ok(DownMsg::Pause) => {
                let break_loop = loop {
                    match $rx.recv() {
                        Ok(DownMsg::Stop) => break true,
                        Ok(DownMsg::Pause) => {}
                        Ok(DownMsg::Continue) => break false,
                        Ok(DownMsg::Other(x)) => {},
                        Err(_) => break true
                    }
                };

                if break_loop {
                    break
                } else {
                    None
                }
            }
            Ok(DownMsg::Continue) => None,
            Ok(DownMsg::Other(x)) => Some(x),
            Err(TryRecvError::Disconnected) => break,
            Err(TryRecvError::Empty) => None,
        }
    }}
}
