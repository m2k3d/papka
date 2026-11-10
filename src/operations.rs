//! Два рабочих потока с ограниченными очередями; файловые действия добавляются отдельно.

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use async_channel::{Receiver, Sender, TrySendError};

pub type OperationId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationState { Queued, Running, AwaitingDecision, Stopping, Finished }

#[derive(Debug, Clone)]
pub enum Outcome { Success, Failed(String), Interrupted }

/// Частичный результат не является успехом и не означает автоматического отката.
#[derive(Debug, Clone)]
pub struct Completion {
    pub outcome: Outcome,
    pub completed: Option<u64>,
    pub remaining: Option<u64>,
    pub undo_available: bool,
}

impl Completion {
    fn interrupted() -> Self {
        Self { outcome: Outcome::Interrupted, completed: Some(0), remaining: None, undo_available: false }
    }
}

#[derive(Debug, Clone)]
pub enum Event {
    Started(OperationId),
    Progress { id: OperationId, completed: u64, total: Option<u64> },
    Finished { id: OperationId, completion: Completion },
}

#[derive(Debug, Clone)]
pub struct Cancellation(Arc<AtomicBool>);

impl Cancellation {
    pub fn request(&self) { self.0.store(true, Ordering::Release); }
    pub fn is_requested(&self) -> bool { self.0.load(Ordering::Acquire) }
}

pub struct Context {
    cancellation: Cancellation,
    shutdown: Arc<AtomicBool>,
    events: Sender<Event>,
    id: OperationId,
    last_progress: Option<Instant>,
}

impl Context {
    pub fn is_cancelled(&self) -> bool {
        self.cancellation.is_requested()
            || self.shutdown.load(Ordering::Acquire)
            || self.events.is_closed()
    }

    /// Промежуточный прогресс объединяется, итог отправляется надёжно отдельно.
    pub fn progress(&mut self, completed: u64, total: Option<u64>) {
        let now = Instant::now();
        if self.last_progress.is_some_and(|last| now.duration_since(last) < Duration::from_millis(100)) {
            return;
        }
        if self.events.try_send(Event::Progress { id: self.id, completed, total }).is_ok() {
            self.last_progress = Some(now);
        }
    }
}

type Work = Box<dyn FnOnce(&mut Context) -> Completion + Send + 'static>;
struct Job { id: OperationId, cancellation: Cancellation, work: Work }

#[derive(Debug, Clone, Copy)]
pub enum SubmitError { QueueFull, Stopped, IdExhausted }

impl std::fmt::Display for SubmitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::QueueFull => "Очередь операций заполнена.",
            Self::Stopped => "Фоновое выполнение остановлено.",
            Self::IdExhausted => "Исчерпаны идентификаторы операций.",
        })
    }
}
impl std::error::Error for SubmitError {}

#[derive(Debug, Clone)]
pub struct Ticket { pub id: OperationId, pub cancellation: Cancellation }

struct Inner {
    reads: Sender<Job>,
    pending_reads: Receiver<Job>,
    mutations: Sender<Job>,
    events: Sender<Event>,
    latest_read: Mutex<Option<Cancellation>>,
    next_id: AtomicU64,
    shutdown: Arc<AtomicBool>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        self.reads.close();
        self.mutations.close();
    }
}

#[derive(Clone)]
pub struct Background(Arc<Inner>);

impl std::fmt::Debug for Background {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Background").finish_non_exhaustive()
    }
}

impl Background {
    pub fn start() -> std::io::Result<(Self, Receiver<Event>)> {
        let (reads, read_jobs) = async_channel::bounded(1);
        let (mutations, mutation_jobs) = async_channel::bounded(32);
        let (events, receiver) = async_channel::bounded(64);
        let shutdown = Arc::new(AtomicBool::new(false));
        let inner = Arc::new(Inner {
            reads, pending_reads: read_jobs.clone(), mutations, events: events.clone(),
            latest_read: Mutex::new(None), next_id: AtomicU64::new(1), shutdown: shutdown.clone(),
        });
        spawn_worker("papka-read", read_jobs, events.clone(), shutdown.clone())?;
        spawn_worker("papka-operations", mutation_jobs, events, shutdown)?;
        Ok((Self(inner), receiver))
    }

    /// Заменяет ожидающее чтение и запрашивает остановку предыдущего.
    /// Вызывающая сторона должна отбрасывать устаревшие результаты по id.
    pub fn read_latest(&self, work: impl FnOnce(&mut Context) -> Completion + Send + 'static) -> Result<Ticket, SubmitError> {
        let mut latest = self.0.latest_read.lock().unwrap_or_else(|error| error.into_inner());
        let (job, ticket) = self.job(Box::new(work))?;
        if let Some(previous) = latest.take() { previous.request(); }
        let _ = self.0.pending_reads.try_recv();
        self.0.reads.try_send(job).map_err(map_send_error)?;
        *latest = Some(ticket.cancellation.clone());
        Ok(ticket)
    }

    pub fn submit(&self, work: impl FnOnce(&mut Context) -> Completion + Send + 'static) -> Result<Ticket, SubmitError> {
        let (job, ticket) = self.job(Box::new(work))?;
        self.0.mutations.try_send(job).map_err(map_send_error)?;
        Ok(ticket)
    }

    /// Только запрос остановки: системный вызов может завершиться позже.
    pub fn shutdown(&self) {
        self.0.shutdown.store(true, Ordering::Release);
        self.0.reads.close();
        self.0.mutations.close();
    }

    fn job(&self, work: Work) -> Result<(Job, Ticket), SubmitError> {
        // После потери получателя нельзя принимать задания без возможности
        // сообщить об их результате, даже если рабочий поток ещё не заметил разрыв.
        if self.0.events.is_closed() {
            self.shutdown();
            return Err(SubmitError::Stopped);
        }
        if self.0.shutdown.load(Ordering::Acquire) { return Err(SubmitError::Stopped); }
        let id = self.0.next_id.try_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .map_err(|_| SubmitError::IdExhausted)?;
        let cancellation = Cancellation(Arc::new(AtomicBool::new(false)));
        let ticket = Ticket { id, cancellation: cancellation.clone() };
        Ok((Job { id, cancellation, work }, ticket))
    }
}

fn map_send_error(error: TrySendError<Job>) -> SubmitError {
    match error { TrySendError::Full(_) => SubmitError::QueueFull, TrySendError::Closed(_) => SubmitError::Stopped }
}

fn spawn_worker(name: &str, jobs: Receiver<Job>, events: Sender<Event>, shutdown: Arc<AtomicBool>) -> std::io::Result<()> {
    std::thread::Builder::new().name(name.into()).spawn(move || {
        while let Ok(job) = jobs.recv_blocking() {
            let mut context = Context {
                id: job.id, cancellation: job.cancellation, shutdown: shutdown.clone(),
                events: events.clone(), last_progress: None,
            };
            if events.send_blocking(Event::Started(job.id)).is_err() { break; }
            let completion = if context.is_cancelled() {
                Completion::interrupted()
            } else {
                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (job.work)(&mut context))) {
                    Ok(completion) => completion,
                    Err(_) => Completion {
                        outcome: Outcome::Failed("Сбой фонового задания; результат требует проверки.".into()),
                        completed: None, remaining: None, undo_available: false,
                    },
                }
            };
            if events.send_blocking(Event::Finished { id: job.id, completion }).is_err() { break; }
        }
        // В том числе закрываем копию получателя, удерживаемую pending_reads.
        // После выхода потока отправитель больше не должен принимать задания.
        jobs.close();
    })?;
    Ok(())
}
