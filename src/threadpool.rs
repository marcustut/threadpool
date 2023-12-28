#[cfg(feature = "dashmap")]
use dashmap::DashMap;
#[cfg(not(feature = "dashmap"))]
use std::collections::HashMap;

use std::sync::Arc;

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("Worker {0} is already running")]
    WorkerAlreadyExist(String),

    #[error("Worker {0} not found")]
    WorkerNotFound(String),

    #[error("Failed to stop worker {} a thread: {:?}", .worker_id, .error)]
    StopError { worker_id: String, error: String },

    #[error("Failed to send shutdown signal to worker {0:?}")]
    SendShutdownSignal(#[from] std::sync::mpsc::SendError<()>),
}

pub type Result<T> = core::result::Result<T, Error>;

pub struct ThreadPool<Id: std::fmt::Debug + Clone + Eq + std::hash::Hash, State: Clone, Return> {
    #[cfg(not(feature = "dashmap"))]
    workers: HashMap<Id, Worker<Id, State, Return>>,
    #[cfg(feature = "dashmap")]
    workers: DashMap<Id, Worker<Id, State, Return>>,
    state: State,
}

impl<Id: std::fmt::Debug + Clone + Eq + std::hash::Hash, State: Clone, Return>
    ThreadPool<Id, State, Return>
{
    pub fn new(state: State) -> Self {
        Self {
            #[cfg(not(feature = "dashmap"))]
            workers: HashMap::new(),
            #[cfg(feature = "dashmap")]
            workers: DashMap::new(),
            state,
        }
    }

    pub fn ids(&self) -> Vec<Id> {
        #[cfg(not(feature = "dashmap"))]
        let ids = self.workers.keys().cloned().collect();
        #[cfg(feature = "dashmap")]
        let ids = self.workers.iter().map(|r| r.id.clone()).collect();
        ids
    }

    #[cfg(not(feature = "dashmap"))]
    pub fn spawn(
        &mut self,
        id: Id,
        handle: Arc<
            dyn Fn(Id, State, std::sync::mpsc::Receiver<()>) -> std::thread::JoinHandle<Return>
                + Send
                + Sync,
        >,
    ) -> Result<()> {
        if self.workers.contains_key(&id) {
            return Err(Error::WorkerAlreadyExist(format!("{:?}", id)));
        }

        tracing::info!("Spawning worker {:?}...", id);
        self.workers
            .insert(id.clone(), Worker::new(id, self.state.clone(), handle));

        Ok(())
    }

    #[cfg(feature = "dashmap")]
    pub fn spawn(
        &self,
        id: Id,
        handle: Arc<
            dyn Fn(Id, State, std::sync::mpsc::Receiver<()>) -> std::thread::JoinHandle<Return>
                + Send
                + Sync,
        >,
    ) -> Result<()> {
        if self.workers.contains_key(&id) {
            return Err(Error::WorkerAlreadyExist(format!("{:?}", id)));
        }

        tracing::info!("Spawning worker {:?}...", id);
        self.workers
            .insert(id.clone(), Worker::new(id, self.state.clone(), handle));

        Ok(())
    }

    #[cfg(not(feature = "dashmap"))]
    pub fn stop(&mut self, id: Id) -> Result<()> {
        match self.workers.remove(&id) {
            Some(worker) => worker.stop(),
            None => Err(Error::WorkerNotFound(format!("{:?}", id))),
        }
    }

    #[cfg(feature = "dashmap")]
    pub fn stop(&self, id: Id) -> Result<()> {
        match self.workers.remove(&id) {
            Some((_, worker)) => worker.stop(),
            None => Err(Error::WorkerNotFound(format!("{:?}", id))),
        }
    }
}

impl<Id: std::fmt::Debug + Clone + Eq + std::hash::Hash, State: Clone, Return> Drop
    for ThreadPool<Id, State, Return>
{
    fn drop(&mut self) {
        tracing::warn!("ThreadPool is being dropped...");
        #[cfg(not(feature = "dashmap"))]
        for (_, worker) in self.workers.drain() {
            worker.stop().unwrap();
        }
        #[cfg(feature = "dashmap")]
        for worker in &self.workers {
            let (_, worker) = self.workers.remove(&worker.id).unwrap();
            worker.stop().unwrap();
        }
    }
}

struct Worker<Id: std::fmt::Debug + Clone, State, Return> {
    id: Id,
    thread: std::thread::JoinHandle<Return>,
    phantom: std::marker::PhantomData<State>,
    shutdown_tx: std::sync::mpsc::Sender<()>,
}

impl<Id: std::fmt::Debug + Clone, State, Return> Worker<Id, State, Return> {
    fn new(
        id: Id,
        state: State,
        handle: Arc<
            dyn Fn(Id, State, std::sync::mpsc::Receiver<()>) -> std::thread::JoinHandle<Return>
                + Send
                + Sync,
        >,
    ) -> Self {
        let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel();
        Self {
            id: id.clone(),
            thread: handle(id, state, shutdown_rx),
            phantom: std::marker::PhantomData,
            shutdown_tx,
        }
    }

    fn stop(self) -> Result<()> {
        tracing::info!("Stopping worker {:?}...", self.id);
        self.shutdown_tx.send(())?;
        self.thread.join().map_err(|e| Error::StopError {
            worker_id: format!("{:?}", self.id),
            error: format!("{:?}", e),
        })?;
        tracing::info!("Worker {:?} shutdown successfully", self.id);
        Ok(())
    }
}
