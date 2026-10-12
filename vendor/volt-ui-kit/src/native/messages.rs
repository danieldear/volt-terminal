//! Bounded worker-to-UI messages. No executor, timer or idle polling.
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

const CAPACITY: usize = 256;
const BATCH: usize = 64;
type Wake = Arc<dyn Fn() -> bool + Send + Sync>;
struct State<M> {
    queue: VecDeque<M>,
    wake: Option<Wake>,
    pending: bool,
    serviced: bool,
    closed: bool,
}
struct Queue<M>(Mutex<State<M>>);
/// Nonblocking enqueue failure. Messages are consumed by `send`, even on failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiSendError {
    Full,
    Closed,
}
impl std::fmt::Display for UiSendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "UI message queue {self:?}")
    }
}
impl std::error::Error for UiSendError {}
/// Cloneable typed sender. State and view functions never leave the UI thread.
/// Capacity is 256 messages; the host handles at most 64 per wake and composes
/// once for the batch. Use application generations/tokens to discard stale jobs.
pub struct UiSender<M> {
    queue: Arc<Queue<M>>,
}
impl<M> Clone for UiSender<M> {
    fn clone(&self) -> Self {
        Self {
            queue: self.queue.clone(),
        }
    }
}
impl<M> std::fmt::Debug for UiSender<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UiSender").finish_non_exhaustive()
    }
}
impl<M: Send + 'static> UiSender<M> {
    /// Enqueue without waiting for queue space. A closed window rejects late work.
    pub fn send(&self, message: M) -> Result<(), UiSendError> {
        let wake = {
            let mut state = self.queue.0.lock().unwrap_or_else(|p| p.into_inner());
            if state.closed {
                return Err(UiSendError::Closed);
            }
            if state.queue.len() >= CAPACITY {
                return Err(UiSendError::Full);
            }
            state.queue.push_back(message);
            if !state.pending && state.wake.is_some() {
                state.pending = true;
                state.wake.clone()
            } else {
                None
            }
        };
        if !self.queue.wake(wake) {
            return Err(UiSendError::Closed);
        }
        Ok(())
    }
    pub fn is_closed(&self) -> bool {
        self.queue
            .0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .closed
    }
}
impl<M> Queue<M> {
    fn wake(&self, wake: Option<Wake>) -> bool {
        if wake.is_some_and(|wake| !wake()) {
            self.close();
            return false;
        }
        true
    }
    fn close(&self) {
        // Drop message payloads outside the mutex: they can own senders/drop hooks.
        let queue = {
            let mut state = self.0.lock().unwrap_or_else(|p| p.into_inner());
            state.closed = true;
            state.pending = false;
            state.serviced = false;
            state.wake = None;
            std::mem::take(&mut state.queue)
        };
        drop(queue);
    }
}
pub(super) trait Messages<C> {
    fn connect(&mut self, wake: Wake);
    fn drain(&mut self, context: &mut C) -> bool;
    fn finish_turn(&mut self);
    fn close(&mut self);
}
type MessageUpdate<C, M> = Box<dyn FnMut(&mut C, M)>;
struct Pump<C, M> {
    queue: Arc<Queue<M>>,
    update: MessageUpdate<C, M>,
}
impl<C, M> Drop for Pump<C, M> {
    fn drop(&mut self) {
        self.queue.close();
    }
}
impl<C, M: Send + 'static> Messages<C> for Pump<C, M> {
    fn connect(&mut self, wake: Wake) {
        let wake = {
            let mut state = self.queue.0.lock().unwrap_or_else(|p| p.into_inner());
            state.wake = Some(wake);
            if !state.queue.is_empty() && !state.pending {
                state.pending = true;
                state.wake.clone()
            } else {
                None
            }
        };
        self.queue.wake(wake);
    }
    fn drain(&mut self, context: &mut C) -> bool {
        let batch = {
            let mut state = self.queue.0.lock().unwrap_or_else(|p| p.into_inner());
            if state.closed || state.serviced {
                return false;
            }
            // macOS winit drains receiver.try_iter() before rendering/input.
            // Keep the wake latched for the WHOLE turn, including user code.
            state.serviced = true;
            state.pending = true;
            let len = state.queue.len().min(BATCH);
            state.queue.drain(..len).collect::<Vec<_>>()
        };
        let changed = !batch.is_empty();
        for message in batch {
            (self.update)(context, message);
        }
        changed
    }
    fn finish_turn(&mut self) {
        let wake = {
            let mut state = self.queue.0.lock().unwrap_or_else(|p| p.into_inner());
            if state.closed || !state.serviced {
                return;
            }
            state.serviced = false;
            state.pending = !state.queue.is_empty();
            if state.pending {
                state.wake.clone()
            } else {
                None
            }
        };
        self.queue.wake(wake);
    }
    fn close(&mut self) {
        self.queue.close();
    }
}
pub(super) fn channel<C: 'static, M: Send + 'static>(
    update: impl FnMut(&mut C, M) + 'static,
) -> (UiSender<M>, Box<dyn Messages<C>>) {
    let queue = Arc::new(Queue(Mutex::new(State {
        queue: VecDeque::new(),
        wake: None,
        pending: false,
        serviced: false,
        closed: false,
    })));
    (
        UiSender {
            queue: queue.clone(),
        },
        Box::new(Pump {
            queue,
            update: Box::new(update),
        }),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    #[test]
    fn bounded_coalesced_batches_preserve_order_and_close() {
        let (sender, mut pump) = channel(|values: &mut Vec<usize>, n| values.push(n));
        for n in 0..CAPACITY {
            sender.send(n).unwrap();
        }
        assert_eq!(sender.send(999), Err(UiSendError::Full));
        let wakes = Arc::new(AtomicUsize::new(0));
        let count = wakes.clone();
        pump.connect(Arc::new(move || {
            count.fetch_add(1, Ordering::SeqCst);
            true
        }));
        assert_eq!(wakes.load(Ordering::SeqCst), 1);
        let mut values = vec![];
        for i in 1..=4 {
            assert!(pump.drain(&mut values));
            assert_eq!(values.len(), i * BATCH);
            pump.finish_turn();
        }
        assert_eq!(values, (0..CAPACITY).collect::<Vec<_>>());
        assert_eq!(wakes.load(Ordering::SeqCst), 4);
        assert!(!pump.drain(&mut values));
        pump.finish_turn();
        sender.send(999).unwrap();
        assert_eq!(wakes.load(Ordering::SeqCst), 5);
        drop(pump);
        assert!(sender.is_closed());
        assert_eq!(sender.send(0), Err(UiSendError::Closed));
    }
    #[test]
    fn reentrant_producer_cannot_extend_a_winit_try_iter_turn() {
        let shared = Arc::new(Mutex::new(None::<UiSender<usize>>));
        let producer = shared.clone();
        let (sender, mut pump) = channel(move |count: &mut usize, n| {
            *count += 1;
            producer
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .send(n + 1)
                .unwrap();
        });
        *shared.lock().unwrap() = Some(sender.clone());
        let (tx, rx) = std::sync::mpsc::channel();
        pump.connect(Arc::new(move || tx.send(()).is_ok()));
        sender.send(0).unwrap();
        let mut count = 0;
        // Mirrors locked winit's macOS drain before redraw/AboutToWait.
        let mut events = 0;
        for _ in rx.try_iter() {
            events += 1;
            assert!(events < 2);
            pump.drain(&mut count);
        }
        assert_eq!(events, 1);
        assert_eq!(count, 1);
        assert!(!pump.drain(&mut count));
        pump.finish_turn(); // Only here may another native turn be queued.
        assert_eq!(rx.try_iter().count(), 1);
        pump.close();
        assert_eq!(sender.send(99), Err(UiSendError::Closed));
        assert!(!pump.drain(&mut count));
        assert_eq!(count, 1);
    }
    #[test]
    fn concurrent_workers_never_lose_wake_or_messages() {
        let (sender, mut pump) = channel(|values: &mut Vec<usize>, n| values.push(n));
        let (tx, rx) = std::sync::mpsc::channel();
        pump.connect(Arc::new(move || tx.send(()).is_ok()));
        let workers: Vec<_> = (0..4)
            .map(|worker| {
                let sender = sender.clone();
                std::thread::spawn(move || {
                    for n in 0..100 {
                        let item = worker * 100 + n;
                        loop {
                            match sender.send(item) {
                                Ok(()) => break,
                                Err(UiSendError::Full) => std::thread::yield_now(),
                                Err(e) => panic!("{e}"),
                            }
                        }
                    }
                })
            })
            .collect();
        let mut values = vec![];
        while values.len() < 400 {
            rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap();
            pump.drain(&mut values);
            pump.finish_turn();
        }
        for worker in workers {
            worker.join().unwrap();
        }
        values.sort_unstable();
        assert_eq!(values, (0..400).collect::<Vec<_>>());
    }
    #[test]
    fn wake_failure_closes_queue_and_callbacks_can_send() {
        let (sender, mut pump) = channel(|_: &mut (), _: u8| {});
        pump.connect(Arc::new(|| false));
        assert_eq!(sender.send(1), Err(UiSendError::Closed));
        let shared = Arc::new(Mutex::new(None::<UiSender<u8>>));
        let target = shared.clone();
        let (sender, mut pump) = channel(move |v: &mut Vec<u8>, n| {
            v.push(n);
            if n == 1 {
                target.lock().unwrap().as_ref().unwrap().send(2).unwrap();
            }
        });
        *shared.lock().unwrap() = Some(sender.clone());
        pump.connect(Arc::new(|| true));
        sender.send(1).unwrap();
        let mut v = vec![];
        pump.drain(&mut v);
        pump.finish_turn();
        pump.drain(&mut v);
        assert_eq!(v, vec![1, 2]);
    }
}
