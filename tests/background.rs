#[path = "../src/operations.rs"]
#[allow(dead_code)]
mod operations;
#[path = "../src/app.rs"]
#[allow(dead_code)]
mod app;

use operations::{Background, Completion, Event, OperationId, Outcome, SubmitError};
use std::collections::BTreeSet;
use std::sync::{Arc, mpsc};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

const LIMIT: Duration = Duration::from_secs(5);

fn success() -> Completion {
    Completion { outcome: Outcome::Success, completed: Some(1), remaining: Some(0), undo_available: false }
}

fn interrupted(completed: u64) -> Completion {
    Completion { outcome: Outcome::Interrupted, completed: Some(completed), remaining: None, undo_available: false }
}

// Таймаут ограничивает зависание дефектного worker; порядок задают каналы, а не sleep.
fn receive<T>(receiver: &mpsc::Receiver<T>) -> T {
    receiver.recv_timeout(LIMIT).expect("Ожидаемое событие не доставлено")
}

fn forward(events: async_channel::Receiver<Event>) -> mpsc::Receiver<Event> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        while let Ok(event) = events.recv_blocking() {
            if sender.send(event).is_err() { break; }
        }
    });
    receiver
}

struct Harness {
    background: Background,
    events: mpsc::Receiver<Event>,
}

impl Harness {
    fn new() -> Self {
        let (background, events) = Background::start().unwrap();
        Self { background, events: forward(events) }
    }

    fn finished(&self, id: OperationId) -> Completion {
        loop {
            if let Event::Finished { id: found, completion } = receive(&self.events) {
                if found == id { return completion; }
            }
        }
    }
}

impl Drop for Harness {
    fn drop(&mut self) { self.background.shutdown(); }
}

#[test]
fn mutation_queue_is_bounded_and_fifo() {
    let harness = Harness::new();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let first = harness.background.submit(move |_| {
        entered_tx.send(()).unwrap();
        receive(&release_rx);
        success()
    }).unwrap();
    receive(&entered_rx);
    let mut ids = vec![first.id];
    for _ in 0..32 {
        ids.push(harness.background.submit(|_| success()).unwrap().id);
    }
    assert!(matches!(harness.background.submit(|_| success()), Err(SubmitError::QueueFull)));
    release_tx.send(()).unwrap();
    let mut started = Vec::new();
    let mut finished = Vec::new();
    while finished.len() < ids.len() {
        match receive(&harness.events) {
            Event::Started(id) => {
                assert_eq!(started.len(), finished.len(), "Изменения выполняются одновременно");
                started.push(id);
            }
            Event::Finished { id, completion } => {
                assert!(matches!(completion.outcome, Outcome::Success));
                finished.push(id);
            }
            _ => {}
        }
    }
    assert_eq!(started, ids);
    assert_eq!(finished, ids);
}

#[test]
fn reading_runs_while_mutation_is_blocked() {
    let harness = Harness::new();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let mutation = harness.background.submit(move |_| {
        entered_tx.send(()).unwrap();
        receive(&release_rx);
        success()
    }).unwrap();
    receive(&entered_rx);
    let (read_tx, read_rx) = mpsc::channel();
    let read = harness.background.read_latest(move |_| {
        read_tx.send(std::thread::current().name().map(str::to_owned)).unwrap();
        success()
    }).unwrap();
    assert_eq!(receive(&read_rx).as_deref(), Some("papka-read"));
    assert!(matches!(harness.finished(read.id).outcome, Outcome::Success));
    release_tx.send(()).unwrap();
    assert!(matches!(harness.finished(mutation.id).outcome, Outcome::Success));
}

#[test]
fn latest_read_cancels_active_and_replaces_pending() {
    let harness = Harness::new();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let first = harness.background.read_latest(move |context| {
        entered_tx.send(()).unwrap();
        receive(&release_rx);
        assert!(context.is_cancelled());
        interrupted(2)
    }).unwrap();
    receive(&entered_rx);
    let obsolete_ran = Arc::new(AtomicBool::new(false));
    let flag = obsolete_ran.clone();
    let obsolete = harness.background.read_latest(move |_| {
        flag.store(true, Ordering::SeqCst);
        success()
    }).unwrap();
    let latest = harness.background.read_latest(|_| success()).unwrap();
    assert!(first.cancellation.is_requested());
    assert!(obsolete.cancellation.is_requested());
    release_tx.send(()).unwrap();
    let mut completed = BTreeSet::new();
    while !completed.contains(&latest.id) {
        match receive(&harness.events) {
            Event::Started(id) => assert_ne!(id, obsolete.id),
            Event::Finished { id, completion } => {
                assert_ne!(id, obsolete.id);
                if id == first.id {
                    assert!(matches!(completion.outcome, Outcome::Interrupted));
                    assert_eq!(completion.completed, Some(2));
                }
                completed.insert(id);
            }
            _ => {}
        }
    }
    assert!(completed.contains(&first.id));
    assert!(!obsolete_ran.load(Ordering::SeqCst));
}

#[test]
fn cancellation_skips_queued_work_and_preserves_partial_result() {
    let harness = Harness::new();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let first = harness.background.submit(move |context| {
        entered_tx.send(()).unwrap();
        receive(&release_rx);
        assert!(context.is_cancelled());
        interrupted(3)
    }).unwrap();
    receive(&entered_rx);
    let queued = harness.background.submit(|_| panic!("Прерванное задание не должно выполняться")).unwrap();
    first.cancellation.request();
    queued.cancellation.request();
    release_tx.send(()).unwrap();
    let completion = harness.finished(first.id);
    assert!(matches!(completion.outcome, Outcome::Interrupted));
    assert_eq!(completion.completed, Some(3));
    let completion = harness.finished(queued.id);
    assert!(matches!(completion.outcome, Outcome::Interrupted));
    assert_eq!(completion.completed, Some(0));
    assert!(!completion.undo_available);
}

#[test]
fn shutdown_cancels_active_and_queued_and_rejects_new_jobs() {
    let harness = Harness::new();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let first = harness.background.submit(move |context| {
        entered_tx.send(()).unwrap();
        receive(&release_rx);
        assert!(context.is_cancelled());
        interrupted(1)
    }).unwrap();
    receive(&entered_rx);
    let queued = harness.background.submit(|_| panic!("Работа после shutdown")).unwrap();
    harness.background.shutdown();
    assert!(matches!(harness.background.submit(|_| success()), Err(SubmitError::Stopped)));
    assert!(matches!(harness.background.read_latest(|_| success()), Err(SubmitError::Stopped)));
    release_tx.send(()).unwrap();
    assert!(matches!(harness.finished(first.id).outcome, Outcome::Interrupted));
    assert!(matches!(harness.finished(queued.id).outcome, Outcome::Interrupted));
}

#[test]
fn panic_reports_unknown_result_and_worker_continues() {
    let harness = Harness::new();
    let failed = harness.background.submit(|_| panic!("Тестовая паника")).unwrap();
    let next = harness.background.submit(|_| success()).unwrap();
    let completion = harness.finished(failed.id);
    assert!(matches!(completion.outcome, Outcome::Failed(_)));
    assert_eq!(completion.completed, None);
    assert_eq!(completion.remaining, None);
    assert!(!completion.undo_available);
    assert!(matches!(harness.finished(next.id).outcome, Outcome::Success));
}

#[test]
fn progress_is_throttled_and_final_result_is_delivered() {
    let harness = Harness::new();
    let (elapsed_tx, elapsed_rx) = mpsc::channel();
    let ticket = harness.background.submit(move |context| {
        let start = Instant::now();
        for value in 0..1000 { context.progress(value, Some(1000)); }
        elapsed_tx.send(start.elapsed()).unwrap();
        success()
    }).unwrap();
    let elapsed = receive(&elapsed_rx);
    let mut progress = 0u128;
    loop {
        match receive(&harness.events) {
            Event::Progress { id, completed, total } => {
                assert_eq!(id, ticket.id);
                assert!(completed < 1000);
                assert_eq!(total, Some(1000));
                progress += 1;
            }
            Event::Finished { id, completion } => {
                assert_eq!(id, ticket.id);
                assert!(matches!(completion.outcome, Outcome::Success));
                break;
            }
            _ => {}
        }
    }
    assert!(progress >= 1);
    assert!(progress <= 1 + elapsed.as_millis() / 100);
}

#[test]
fn slow_consumer_keeps_all_terminal_events() {
    let (background, events) = Background::start().unwrap();
    let (first_tx, first_rx) = mpsc::channel();
    let (first_release_tx, first_release_rx) = mpsc::channel();
    let first = background.submit(move |_| {
        first_tx.send(()).unwrap();
        receive(&first_release_rx);
        success()
    }).unwrap();
    receive(&first_rx);
    let (last_tx, last_rx) = mpsc::channel();
    let mut ids = vec![first.id];
    for _ in 0..31 { ids.push(background.submit(|_| success()).unwrap().id); }
    ids.push(background.submit(move |_| {
        last_tx.send(()).unwrap();
        success()
    }).unwrap().id);
    first_release_tx.send(()).unwrap();
    // Ждём наблюдаемое заполнение канала: Started 33-го задания блокируется,
    // пока получатель не начнёт читать. Таймаут лишь защищает от зависания.
    let deadline = Instant::now() + LIMIT;
    while events.len() < 64 {
        assert!(Instant::now() < deadline, "Канал событий не заполнился");
        std::thread::yield_now();
    }
    assert!(matches!(last_rx.try_recv(), Err(mpsc::TryRecvError::Empty)));
    let forwarded = forward(events);
    receive(&last_rx);
    let mut started = Vec::new();
    let mut finished = Vec::new();
    while finished.len() < ids.len() {
        match receive(&forwarded) {
            Event::Started(id) => started.push(id),
            Event::Finished { id, .. } => finished.push(id),
            _ => {}
        }
    }
    assert_eq!(started, ids);
    assert_eq!(finished, ids);
    background.shutdown();
}

#[test]
fn app_tracks_running_until_terminal_event() {
    let mut app = app::App::default();
    assert!(!app.is_busy());
    app.apply(Event::Started(1));
    app.apply(Event::Started(2));
    app.apply(Event::Progress { id: 1, completed: 4, total: Some(10) });
    app.apply(Event::Finished { id: 1, completion: interrupted(4) });
    assert!(app.is_busy());
    app.apply(Event::Finished { id: 2, completion: success() });
    assert!(!app.is_busy());
}

struct Dropped(mpsc::Sender<()>);
impl Drop for Dropped {
    fn drop(&mut self) { let _ = self.0.send(()); }
}

#[test]
fn disconnected_event_consumer_stops_accepting_reads() {
    let (background, events) = Background::start().unwrap();
    drop(events);
    let (dropped_tx, dropped_rx) = mpsc::channel();
    let guard = Dropped(dropped_tx);
    // Worker не может отправить Started, поэтому выходит и уничтожает замыкание.
    background.read_latest(move |_| {
        let _guard = guard;
        panic!("Без получателя событий работа не должна запускаться")
    }).unwrap();
    receive(&dropped_rx);
    let result = background.read_latest(|_| success());
    background.shutdown();
    assert!(matches!(result, Err(SubmitError::Stopped)),
        "Чтение принято после выхода рабочего потока: {result:?}");
}
