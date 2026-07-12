//! Bounded worker pool, channels, and poison-aware shared state.

use std::sync::{mpsc, Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};

type Job = Box<dyn FnOnce() + Send + 'static>;
enum Message { Run(Job), Stop }

#[derive(Debug, Clone, PartialEq, Eq)] pub enum PoolError { Closed, InvalidSize }

pub struct ThreadPool { sender: Option<mpsc::SyncSender<Message>>, workers: Vec<JoinHandle<()>> }
impl ThreadPool {
    pub fn new(size: usize, queue_capacity: usize) -> Result<Self, PoolError> {
        if size == 0 { return Err(PoolError::InvalidSize); }
        let (sender, receiver) = mpsc::sync_channel::<Message>(queue_capacity); let receiver = Arc::new(Mutex::new(receiver)); let mut workers = Vec::with_capacity(size);
        for index in 0..size { let receiver = Arc::clone(&receiver); workers.push(thread::Builder::new().name(format!("titan-worker-{index}")).spawn(move || loop { let message = match receiver.lock() { Ok(lock) => lock.recv(), Err(poisoned) => poisoned.into_inner().recv() }; match message { Ok(Message::Run(job)) => { let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job)); }, Ok(Message::Stop) | Err(_) => break } }).map_err(|_| PoolError::Closed)?); }
        Ok(Self { sender: Some(sender), workers })
    }
    pub fn execute<F>(&self, task: F) -> Result<(), PoolError> where F: FnOnce() + Send + 'static { self.sender.as_ref().ok_or(PoolError::Closed)?.send(Message::Run(Box::new(task))).map_err(|_| PoolError::Closed) }
    pub fn size(&self) -> usize { self.workers.len() }
}
impl Drop for ThreadPool { fn drop(&mut self) { if let Some(sender) = self.sender.take() { for _ in &self.workers { let _ = sender.send(Message::Stop); } } for worker in self.workers.drain(..) { let _ = worker.join(); } } }

#[derive(Debug)] pub struct Shared<T>(Arc<Mutex<T>>);
impl<T> Clone for Shared<T> { fn clone(&self) -> Self { Self(Arc::clone(&self.0)) } }
impl<T> Shared<T> { pub fn new(value: T) -> Self { Self(Arc::new(Mutex::new(value))) } pub fn lock(&self) -> MutexGuard<'_, T> { self.0.lock().unwrap_or_else(|p| p.into_inner()) } pub fn map<R>(&self, function: impl FnOnce(&mut T) -> R) -> R { function(&mut self.lock()) } }
pub fn channel<T>() -> (mpsc::Sender<T>, mpsc::Receiver<T>) { mpsc::channel() }
pub fn bounded_channel<T>(capacity: usize) -> (mpsc::SyncSender<T>, mpsc::Receiver<T>) { mpsc::sync_channel(capacity) }

#[cfg(test)] mod tests { use super::*; #[test] fn pool_runs_jobs_and_joins() { let value = Shared::new(0); { let pool = ThreadPool::new(2, 4).unwrap(); for _ in 0..4 { let value = value.clone(); pool.execute(move || value.map(|v| *v += 1)).unwrap(); } } assert_eq!(*value.lock(), 4); } }
