#![cfg(target_os = "linux")]
#![allow(dead_code)] // Модули подключены для проверки файлового API.

#[path = "../src/files.rs"]
mod files;
#[path = "../src/history.rs"]
mod history;
#[path = "../src/operations.rs"]
mod operations;
#[path = "../src/platform.rs"]
mod platform;

use files::{Command, Data, Engine, HistoryItem};
use operations::{Background, Completion, Event, Outcome};
use std::{
    fs,
    os::unix::{
        ffi::OsStringExt,
        fs::{MetadataExt, PermissionsExt, symlink},
    },
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

struct Harness {
    background: Background,
    events: mpsc::Receiver<Event>,
    engine: Arc<Mutex<Engine>>,
    history: Vec<HistoryItem>,
}
impl Harness {
    fn new() -> Self {
        let (background, receiver) = Background::start().unwrap();
        let (sender, events) = mpsc::channel();
        std::thread::spawn(move || {
            while let Ok(event) = receiver.recv_blocking() {
                if sender.send(event).is_err() {
                    break;
                }
            }
        });
        Self {
            background,
            events,
            engine: Arc::new(Mutex::new(Engine::new())),
            history: vec![],
        }
    }
    fn finish(&mut self, id: u64) -> Completion {
        loop {
            match self.events.recv_timeout(Duration::from_secs(15)).unwrap() {
                Event::Data {
                    id: event_id,
                    data: Data::History(items),
                } if event_id == id => self.history = items,
                Event::Finished {
                    id: event_id,
                    completion,
                } if event_id == id => return completion,
                _ => {}
            }
        }
    }
    fn run(&mut self, command: Command) -> Completion {
        let engine = self.engine.clone();
        let ticket = self
            .background
            .submit(move |ctx| engine.lock().unwrap().run(command, ctx))
            .unwrap();
        self.finish(ticket.id)
    }
    fn success(&mut self, command: Command) -> u64 {
        let completion = self.run(command);
        assert!(
            matches!(completion.outcome, Outcome::Success),
            "{completion:?}"
        );
        self.history.last().unwrap().id
    }
}
impl Drop for Harness {
    fn drop(&mut self) {
        self.background.shutdown();
    }
}

fn isolated(scenario: &str) {
    let root = tempfile::tempdir().unwrap();
    for name in ["config", "data", "state", "work"] {
        fs::create_dir(root.path().join(name)).unwrap();
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "isolated_probe", "--ignored", "--nocapture"])
        .env("PAPKA_FILES_SCENARIO", scenario)
        .env("PAPKA_FILES_ROOT", root.path())
        .env("XDG_CONFIG_HOME", root.path().join("config"))
        .env("XDG_DATA_HOME", root.path().join("data"))
        .env("XDG_STATE_HOME", root.path().join("state"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    print!("{}", String::from_utf8_lossy(&output.stdout));
}

#[test]
fn create_copy_reopen_and_undo_preserve_data() {
    isolated("copy");
}
#[test]
fn move_rename_trash_and_undo_restore_names() {
    isolated("move");
}
#[test]
fn changed_copy_and_conflicting_restore_refuse_without_loss() {
    isolated("modified");
}
#[test]
fn partial_copy_and_cancelled_work_report_distinct_results() {
    isolated("partial");
}
#[test]
fn links_non_utf8_and_batched_sorting_round_trip() {
    isolated("listing");
}
#[test]
fn unsafe_trash_refuses_without_deleting_source() {
    isolated("trash_refusal");
}

// Родитель запускает probe с отдельными XDG-путями для каждого сценария.
#[test]
#[ignore]
fn isolated_probe() {
    let root = PathBuf::from(std::env::var_os("PAPKA_FILES_ROOT").unwrap());
    let work = root.join("work");
    let data = root.join("data");
    assert_eq!(
        platform::AppDirectories::resolve().unwrap().history,
        root.join("state/papka")
    );
    assert_eq!(
        platform::AppDirectories::resolve().unwrap().config,
        root.join("config/papka")
    );
    assert_eq!(dirs::data_dir().unwrap(), data);
    // До вызова платформы доказано, что корзина будет домашней и внутри TempDir.
    assert_eq!(
        fs::metadata(&work).unwrap().dev(),
        fs::metadata(&data).unwrap().dev()
    );
    let probe = work.join("probe");
    fs::write(&probe, b"probe").unwrap();
    assert_eq!(
        platform::trash_directory(&probe).unwrap(),
        data.join("Trash/files")
    );
    let scenario = std::env::var("PAPKA_FILES_SCENARIO").unwrap();
    match scenario.as_str() {
        "copy" => {
            let mut h = Harness::new();
            let source = work.join("source");
            h.success(Command::Create(source.clone(), true));
            h.success(Command::Create(source.join("empty"), false));
            fs::write(source.join("payload"), b"persistent content").unwrap();
            let destination = work.join("destination");
            fs::create_dir(&destination).unwrap();
            let id = h.success(Command::Copy(vec![source.clone()], destination.clone()));
            assert_eq!(
                fs::read(destination.join("source/payload")).unwrap(),
                b"persistent content"
            );
            drop(h);
            let mut h = Harness::new();
            h.success(Command::Undo(id));
            assert!(!destination.join("source").exists());
            assert_eq!(
                fs::read(source.join("payload")).unwrap(),
                b"persistent content"
            );
            let trashed: Vec<_> = fs::read_dir(data.join("Trash/files"))
                .unwrap()
                .map(|e| e.unwrap().path())
                .collect();
            assert_eq!(trashed.len(), 1);
            assert_eq!(
                fs::read(trashed[0].join("payload")).unwrap(),
                b"persistent content"
            );
        }
        "move" => {
            let mut h = Harness::new();
            let source = work.join("source");
            fs::write(&source, b"retain").unwrap();
            let destination = work.join("destination");
            fs::create_dir(&destination).unwrap();
            let id = h.success(Command::Move(vec![source.clone()], destination.clone()));
            assert!(!source.exists());
            h.success(Command::Undo(id));
            assert_eq!(fs::read(&source).unwrap(), b"retain");
            let renamed = work.join("renamed");
            let id = h.success(Command::Rename(source.clone(), renamed.clone()));
            assert!(!source.exists());
            h.success(Command::Undo(id));
            assert!(!renamed.exists());
            let id = h.success(Command::Trash(vec![source.clone()]));
            assert!(!source.exists());
            assert_eq!(fs::read_dir(data.join("Trash/files")).unwrap().count(), 1);
            h.success(Command::Undo(id));
            assert_eq!(fs::read(&source).unwrap(), b"retain");
            assert_eq!(fs::read_dir(data.join("Trash/info")).unwrap().count(), 0);
        }
        "modified" => {
            let mut h = Harness::new();
            let source = work.join("source");
            fs::write(&source, b"initial").unwrap();
            let destination = work.join("destination");
            fs::create_dir(&destination).unwrap();
            let id = h.success(Command::Copy(vec![source.clone()], destination.clone()));
            fs::write(destination.join("source"), b"user edit after copy").unwrap();
            let result = h.run(Command::Undo(id));
            assert!(matches!(result.outcome, Outcome::Failed(_)));
            assert_eq!(
                fs::read(destination.join("source")).unwrap(),
                b"user edit after copy"
            );
            assert_eq!(fs::read(&source).unwrap(), b"initial");
            let renamed = work.join("renamed");
            let id = h.success(Command::Rename(source.clone(), renamed.clone()));
            fs::write(&source, b"conflicting file").unwrap();
            assert!(matches!(
                h.run(Command::Undo(id)).outcome,
                Outcome::Failed(_)
            ));
            assert_eq!(fs::read(&source).unwrap(), b"conflicting file");
            assert_eq!(fs::read(&renamed).unwrap(), b"initial");
            let result = h.run(Command::Copy(vec![source.clone()], work.clone()));
            assert!(matches!(result.outcome, Outcome::Failed(_)));
            assert_eq!(fs::read(source).unwrap(), b"conflicting file");
        }
        "partial" => {
            let mut h = Harness::new();
            let good = work.join("good");
            fs::write(&good, b"copied first").unwrap();
            let fifo = work.join("fifo");
            let c = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
            assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
            let destination = work.join("destination");
            fs::create_dir(&destination).unwrap();
            let result = h.run(Command::Copy(vec![good.clone(), fifo], destination.clone()));
            assert!(matches!(result.outcome, Outcome::Failed(_)));
            assert_eq!(result.completed, Some(1));
            assert_eq!(result.remaining, Some(1));
            assert!(result.undo_available);
            let id = h.history.last().unwrap().id;
            let document: serde_json::Value =
                serde_json::from_slice(&fs::read(root.join("state/papka/history.json")).unwrap())
                    .unwrap();
            assert_eq!(
                document["records"].as_array().unwrap().last().unwrap()["finished"]["result"],
                "Partial"
            );
            h.success(Command::Undo(id));
            assert!(!destination.join("good").exists());
            assert_eq!(fs::read(&good).unwrap(), b"copied first");
            let engine = h.engine.clone();
            let (entered, receive) = mpsc::channel();
            let (release, wait) = mpsc::channel();
            let target = work.join("cancelled");
            fs::create_dir(&target).unwrap();
            let target_for_job = target.clone();
            let ticket = h
                .background
                .submit(move |ctx| {
                    entered.send(()).unwrap();
                    wait.recv().unwrap();
                    engine
                        .lock()
                        .unwrap()
                        .run(Command::Copy(vec![good], target_for_job), ctx)
                })
                .unwrap();
            receive.recv_timeout(Duration::from_secs(5)).unwrap();
            ticket.cancellation.request();
            release.send(()).unwrap();
            assert!(matches!(h.finish(ticket.id).outcome, Outcome::Interrupted));
            assert_eq!(fs::read_dir(target).unwrap().count(), 0);
        }
        "listing" => {
            let source = work.join("source");
            fs::create_dir(&source).unwrap();
            let name = std::ffi::OsString::from_vec(b"non-utf8-\xff".to_vec());
            fs::write(source.join(&name), b"binary name").unwrap();
            symlink("missing-target", source.join("broken")).unwrap();
            let destination = work.join("destination");
            fs::create_dir(&destination).unwrap();
            let mut h = Harness::new();
            h.success(Command::Copy(vec![source.clone()], destination.clone()));
            assert_eq!(
                fs::read(destination.join("source").join(&name)).unwrap(),
                b"binary name"
            );
            assert_eq!(
                fs::read_link(destination.join("source/broken")).unwrap(),
                Path::new("missing-target")
            );
            let renamed = work.join("renamed-non-utf8");
            let id = h.success(Command::Rename(source.join(&name), renamed.clone()));
            h.success(Command::Undo(id));
            assert_eq!(fs::read(source.join(&name)).unwrap(), b"binary name");
            let listing = work.join("listing");
            fs::create_dir(&listing).unwrap();
            fs::create_dir(listing.join("z-directory")).unwrap();
            for i in 0..520 {
                fs::write(listing.join(format!("file-{i:03}")), b"x").unwrap();
            }
            let request = listing.clone();
            let ticket = h
                .background
                .read_latest(move |ctx| {
                    let result = files::listing(request, files::Settings::default(), ctx);
                    Completion {
                        outcome: result
                            .map_or_else(|e| Outcome::Failed(e.to_string()), |_| Outcome::Success),
                        completed: None,
                        remaining: None,
                        undo_available: false,
                    }
                })
                .unwrap();
            let mut entries = vec![];
            let mut sizes = vec![];
            let mut began = false;
            let mut ended = false;
            loop {
                match h.events.recv_timeout(Duration::from_secs(5)).unwrap() {
                    Event::Data { id, data } if id == ticket.id => match data {
                        Data::ListingBegin(path) => {
                            assert_eq!(path, listing);
                            began = true;
                        }
                        Data::ListingBatch(batch) => {
                            assert!(began && !ended);
                            sizes.push(batch.len());
                            entries.extend(batch);
                        }
                        Data::ListingEnd(warning) => {
                            assert!(warning.is_none());
                            ended = true;
                        }
                        _ => {}
                    },
                    Event::Finished { id, completion } if id == ticket.id => {
                        assert!(matches!(completion.outcome, Outcome::Success));
                        break;
                    }
                    _ => {}
                }
            }
            assert!(ended);
            assert_eq!(sizes, vec![256, 256, 9]);
            assert!(entries[0].directory);
            assert_eq!(entries[1].name, "file-000");
            assert_eq!(entries.last().unwrap().name, "file-519");
        }
        "trash_refusal" => {
            let source = work.join("retained");
            fs::write(&source, b"must survive").unwrap();
            fs::set_permissions(data.join("Trash"), fs::Permissions::from_mode(0o777)).unwrap();
            let mut h = Harness::new();
            assert!(matches!(
                h.run(Command::Trash(vec![source.clone()])).outcome,
                Outcome::Failed(_)
            ));
            assert_eq!(fs::read(source).unwrap(), b"must survive");
            fs::set_permissions(data.join("Trash"), fs::Permissions::from_mode(0o700)).unwrap();
        }
        _ => panic!("Неизвестный сценарий"),
    }
}
