#![cfg(target_os = "linux")]
#![allow(dead_code, unused_imports)] // Модули подключены для проверки файлового API.

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
        .env("PAPKA_DND_SCENARIO", scenario)
        .env("PAPKA_DND_ROOT", root.path())
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

impl Harness {
    fn snapshot(&mut self, paths: Vec<PathBuf>) -> files::DragSnapshot {
        let ticket = self
            .background
            .submit(move |ctx| {
                ctx.data(Data::DragPrepared(
                    1,
                    files::DragSnapshot::capture(&paths, ctx).map_err(|e| e.to_string()),
                ));
                Completion {
                    outcome: Outcome::Success,
                    completed: None,
                    remaining: None,
                    undo_available: false,
                }
            })
            .unwrap();
        let mut snapshot = None;
        loop {
            match self.events.recv_timeout(Duration::from_secs(15)).unwrap() {
                Event::Data {
                    id,
                    data: Data::DragPrepared(_, result),
                } if id == ticket.id => snapshot = Some(result.unwrap()),
                Event::Finished { id, .. } if id == ticket.id => return snapshot.unwrap(),
                _ => {}
            }
        }
    }
}
#[test]
fn dnd_receive_move_undo() {
    isolated("receive_move_undo");
}
#[test]
fn dnd_receive_move_retained_source() {
    isolated("receive_move_retained_source");
}
#[test]
fn dnd_receive_move_partial_undo() {
    isolated("receive_move_partial_undo");
}
#[test]
fn dnd_receive_move_conflict() {
    isolated("receive_move_conflict");
}
#[test]
fn dnd_receive_move_modified_undo() {
    isolated("receive_move_modified_undo");
}
#[test]
fn dnd_external_cleanup_undo() {
    isolated("external_cleanup_undo");
}
#[test]
fn dnd_external_changed_tree() {
    isolated("external_changed_tree");
}
#[test]
fn dnd_external_source_already_moved() {
    isolated("external_source_already_moved");
}
#[test]
fn dnd_external_unsafe_trash() {
    isolated("external_unsafe_trash");
}
#[test]
fn dnd_snapshot_cancel_and_limits() {
    isolated("snapshot_cancel_and_limits");
}
#[test]
fn dnd_receive_move_links_nonutf8() {
    isolated("receive_move_links_nonutf8");
}
#[test]
fn dnd_receive_move_cross_device_undo() {
    isolated("receive_move_cross_device_undo");
}

#[test]
fn dnd_partial_receive_checkpoint_survives_reopen() {
    isolated("partial_reopen");
}
#[test]
fn dnd_interrupted_receive_checkpoint_undo_preserves_source() {
    isolated("interrupted_checkpoint");
}
#[test]
fn dnd_partial_receive_undo_preserves_edited_copy() {
    isolated("partial_edited");
}
#[test]
fn dnd_partial_receive_undo_refuses_unsafe_trash() {
    isolated("partial_unsafe_trash");
}
#[test]
fn dnd_ordinary_partial_move_still_restores_original() {
    isolated("ordinary_partial_move");
}
#[test]
fn dnd_legacy_checkpoint_without_receive_flag_keeps_guard() {
    isolated("legacy_checkpoint");
}
#[test]
fn dnd_partial_receive_undo_does_not_require_source_parent() {
    isolated("missing_source_parent");
}

// Создаём настоящее частичное копирование; FIFO не читается и сразу отвергается.
fn partial_receive(h: &mut Harness, source: &Path, dest: &Path, work: &Path) -> u64 {
    let fifo = work.join("partial-fifo");
    let name = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    let result = h.run(Command::ReceiveMove(vec![source.into(), fifo], dest.into()));
    assert!(matches!(result.outcome, Outcome::Failed(_)), "{result:?}");
    assert_eq!((result.completed, result.remaining), (Some(1), Some(1)));
    assert!(result.undo_available);
    h.history.last().unwrap().id
}
fn saved_checkpoint(root: &Path, id: u64) -> (serde_json::Value, serde_json::Value) {
    let document: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("state/papka/history.json")).unwrap()).unwrap();
    let record = document["records"]
        .as_array()
        .unwrap()
        .iter()
        .find(|record| record["id"] == id)
        .unwrap()
        .clone();
    let bytes: Vec<u8> = serde_json::from_value(record["checkpoint"].clone()).unwrap();
    (record, serde_json::from_slice(&bytes).unwrap())
}

#[test]
#[ignore]
fn isolated_probe() {
    let root = PathBuf::from(std::env::var_os("PAPKA_DND_ROOT").unwrap());
    let work = root.join("work");
    let data = root.join("data");
    assert_eq!(
        platform::AppDirectories::resolve().unwrap().history,
        root.join("state/papka")
    );
    assert_eq!(dirs::data_dir().unwrap(), data);
    // Ни один вызов корзины не выполняется до доказанной изоляции на том же томе.
    assert_eq!(
        fs::metadata(&work).unwrap().dev(),
        fs::metadata(&data).unwrap().dev()
    );
    let probe = work.join("trash-isolation-probe");
    fs::write(&probe, b"isolated").unwrap();
    assert_eq!(
        platform::trash_directory(&probe).unwrap(),
        data.join("Trash/files")
    );
    fs::remove_file(probe).unwrap();
    let mut h = Harness::new();
    let source = work.join("source");
    let dest = work.join("destination");
    fs::create_dir(&dest).unwrap();
    fs::write(&source, b"original").unwrap();
    match std::env::var("PAPKA_DND_SCENARIO").unwrap().as_str() {
        "receive_move_undo" => {
            let id = h.success(Command::ReceiveMove(vec![source.clone()], dest.clone()));
            assert_eq!(
                fs::read(&source).unwrap(),
                b"original",
                "До подтверждения источник принадлежит отправителю"
            );
            fs::remove_file(&source).unwrap(); // моделируем подтверждённую очистку отправителем
            h.success(Command::Undo(id));
            assert_eq!(fs::read(source).unwrap(), b"original");
            assert!(!dest.join("source").exists());
        }
        "receive_move_retained_source" => {
            let id = h.success(Command::ReceiveMove(vec![source.clone()], dest.clone()));
            let result = h.run(Command::Undo(id));
            assert!(matches!(result.outcome, Outcome::Failed(_)));
            assert_eq!(fs::read(source).unwrap(), b"original");
            assert_eq!(fs::read(dest.join("source")).unwrap(), b"original");
        }
        "receive_move_partial_undo" => {
            let fifo = work.join("fifo");
            let name = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
            assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
            let result = h.run(Command::ReceiveMove(
                vec![source.clone(), fifo],
                dest.clone(),
            ));
            assert!(matches!(result.outcome, Outcome::Failed(_)));
            assert_eq!((result.completed, result.remaining), (Some(1), Some(1)));
            assert!(result.undo_available);
            let id = h.history.last().unwrap().id;
            let undo = h.run(Command::Undo(id));
            assert!(
                matches!(undo.outcome, Outcome::Success),
                "Отрицательное подтверждение Move оставляет источники; отмена должна убрать подтверждённую копию без требования освободить оригинал: {undo:?}"
            );
            assert_eq!(fs::read(source).unwrap(), b"original");
            assert!(!dest.join("source").exists());
        }
        "receive_move_conflict" => {
            fs::write(dest.join("source"), b"existing user data").unwrap();
            assert!(matches!(
                h.run(Command::ReceiveMove(vec![source.clone()], dest.clone()))
                    .outcome,
                Outcome::Failed(_)
            ));
            assert_eq!(fs::read(source).unwrap(), b"original");
            assert_eq!(
                fs::read(dest.join("source")).unwrap(),
                b"existing user data"
            );
            assert!(h.history.is_empty());
        }
        "receive_move_modified_undo" => {
            let id = h.success(Command::ReceiveMove(vec![source.clone()], dest.clone()));
            fs::remove_file(&source).unwrap();
            fs::write(dest.join("source"), b"user edit").unwrap();
            assert!(matches!(
                h.run(Command::Undo(id)).outcome,
                Outcome::Failed(_)
            ));
            assert_eq!(fs::read(dest.join("source")).unwrap(), b"user edit");
            assert!(!source.exists());
        }
        "external_cleanup_undo" => {
            let snapshot = h.snapshot(vec![source.clone()]);
            fs::copy(&source, dest.join("source")).unwrap();
            let id = h.success(Command::FinishExternalMove(snapshot));
            assert!(!source.exists());
            assert_eq!(fs::read_dir(data.join("Trash/files")).unwrap().count(), 1);
            h.success(Command::Undo(id));
            assert_eq!(fs::read(source).unwrap(), b"original");
            assert_eq!(fs::read(dest.join("source")).unwrap(), b"original");
        }
        "external_changed_tree" => {
            let tree = work.join("tree");
            fs::create_dir(&tree).unwrap();
            fs::write(tree.join("nested"), b"before").unwrap();
            let snapshot = h.snapshot(vec![source.clone(), tree.clone()]);
            fs::write(tree.join("nested"), b"edited").unwrap();
            assert!(matches!(
                h.run(Command::FinishExternalMove(snapshot)).outcome,
                Outcome::Failed(_)
            ));
            assert_eq!(fs::read(source).unwrap(), b"original");
            assert_eq!(fs::read(tree.join("nested")).unwrap(), b"edited");
            assert_eq!(fs::read_dir(data.join("Trash/files")).unwrap().count(), 0);
        }
        "external_source_already_moved" => {
            let snapshot = h.snapshot(vec![source.clone()]);
            fs::rename(&source, dest.join("source")).unwrap();
            assert!(matches!(
                h.run(Command::FinishExternalMove(snapshot)).outcome,
                Outcome::Success
            ));
            assert!(
                h.history.is_empty(),
                "Стороннее перемещение не выдаётся за локальную отменяемую очистку"
            );
            assert_eq!(fs::read(dest.join("source")).unwrap(), b"original");
        }
        "external_unsafe_trash" => {
            let snapshot = h.snapshot(vec![source.clone()]);
            fs::set_permissions(data.join("Trash"), fs::Permissions::from_mode(0o777)).unwrap();
            assert!(matches!(
                h.run(Command::FinishExternalMove(snapshot)).outcome,
                Outcome::Failed(_)
            ));
            assert_eq!(fs::read(source).unwrap(), b"original");
            fs::set_permissions(data.join("Trash"), fs::Permissions::from_mode(0o700)).unwrap();
        }
        "snapshot_cancel_and_limits" => {
            let paths = vec![source.clone(); 101];
            let (reply, result) = mpsc::channel();
            let ticket = h
                .background
                .submit(move |ctx| {
                    assert!(files::DragSnapshot::capture(&[], ctx).is_err());
                    assert!(files::DragSnapshot::capture(&paths, ctx).is_err());
                    reply.send(()).unwrap();
                    Completion {
                        outcome: Outcome::Success,
                        completed: None,
                        remaining: None,
                        undo_available: false,
                    }
                })
                .unwrap();
            h.finish(ticket.id);
            result.recv().unwrap();
            let (entered, receive) = mpsc::channel();
            let (release, wait) = mpsc::channel();
            let ticket = h
                .background
                .submit(move |ctx| {
                    entered.send(()).unwrap();
                    wait.recv().unwrap();
                    assert!(files::DragSnapshot::capture(&[source], ctx).is_err());
                    Completion {
                        outcome: Outcome::Success,
                        completed: None,
                        remaining: None,
                        undo_available: false,
                    }
                })
                .unwrap();
            receive.recv_timeout(Duration::from_secs(5)).unwrap();
            ticket.cancellation.request();
            release.send(()).unwrap();
            h.finish(ticket.id);
        }
        "receive_move_links_nonutf8" => {
            let tree = work.join("tree");
            fs::create_dir(&tree).unwrap();
            let name = std::ffi::OsString::from_vec(b"file-\xff".to_vec());
            fs::write(tree.join(&name), b"binary name").unwrap();
            symlink("missing", tree.join("broken-link")).unwrap();
            let id = h.success(Command::ReceiveMove(vec![tree.clone()], dest.clone()));
            assert_eq!(
                fs::read_link(dest.join("tree/broken-link")).unwrap(),
                Path::new("missing")
            );
            fs::remove_dir_all(&tree).unwrap();
            h.success(Command::Undo(id));
            assert_eq!(fs::read(tree.join(name)).unwrap(), b"binary name");
            assert_eq!(
                fs::read_link(tree.join("broken-link")).unwrap(),
                Path::new("missing")
            );
        }
        "receive_move_cross_device_undo" => {
            // Назначение и его корзина остаются в изолированном root; второй том нужен только для восстановленного оригинала.
            let other = match tempfile::tempdir_in("/dev/shm") {
                Ok(value)
                    if fs::metadata(value.path()).unwrap().dev()
                        != fs::metadata(&work).unwrap().dev() =>
                {
                    value
                }
                _ => {
                    println!("SKIP: нет доступного второго временного тома");
                    return;
                }
            };
            let original = other.path().join("cross-source");
            fs::write(&original, b"cross device").unwrap();
            let id = h.success(Command::ReceiveMove(vec![original.clone()], dest.clone()));
            fs::remove_file(&original).unwrap();
            h.success(Command::Undo(id));
            assert_eq!(fs::read(original).unwrap(), b"cross device");
            assert!(!dest.join("cross-source").exists());
            assert_eq!(fs::read_dir(data.join("Trash/files")).unwrap().count(), 1);
        }
        "partial_reopen" => {
            let id = partial_receive(&mut h, &source, &dest, &work);
            let (record, checkpoint) = saved_checkpoint(&root, id);
            assert_eq!(record["finished"]["result"], "Partial");
            assert_eq!(checkpoint["receive_move"], true);
            assert_eq!(checkpoint["items"][0]["done"], true);
            drop(h);
            let mut reopened = Harness::new();
            reopened.success(Command::Undo(id));
            assert_eq!(fs::read(&source).unwrap(), b"original");
            assert!(!dest.join("source").exists());
            let (_, checkpoint) = saved_checkpoint(&root, id);
            assert_eq!(checkpoint["receive_move"], true);
            assert_eq!(checkpoint["items"][0]["undone"], true);
            assert_eq!(fs::read_dir(data.join("Trash/files")).unwrap().count(), 1);
        }
        "interrupted_checkpoint" => {
            let partial_id = partial_receive(&mut h, &source, &dest, &work);
            let (_, checkpoint) = saved_checkpoint(&root, partial_id);
            drop(h);
            // Fixture завершённого Interrupted с одним подтверждённым элементом.
            // Это проверка решения Undo по сохранённому состоянию, а не реальный Stop посреди I/O.
            let mut journal =
                history::Journal::open(&root.join("state/papka"), files::now()).unwrap();
            let bytes = serde_json::to_vec(&checkpoint).unwrap();
            let id = journal
                .begin(history::Action::Move, bytes.clone(), files::now())
                .unwrap();
            journal
                .finish(id, history::ResultKind::Interrupted, bytes, files::now())
                .unwrap();
            drop(journal);
            let mut reopened = Harness::new();
            reopened.success(Command::Undo(id));
            assert_eq!(fs::read(&source).unwrap(), b"original");
            assert!(!dest.join("source").exists());
            assert_eq!(fs::read_dir(data.join("Trash/files")).unwrap().count(), 1);
            let (record, checkpoint) = saved_checkpoint(&root, id);
            assert_eq!(record["finished"]["result"], "Interrupted");
            assert_eq!(checkpoint["items"][0]["undone"], true);
        }
        "partial_edited" => {
            let id = partial_receive(&mut h, &source, &dest, &work);
            fs::write(dest.join("source"), b"user changed copy").unwrap();
            assert!(matches!(
                h.run(Command::Undo(id)).outcome,
                Outcome::Failed(_)
            ));
            assert_eq!(fs::read(&source).unwrap(), b"original");
            assert_eq!(fs::read(dest.join("source")).unwrap(), b"user changed copy");
            assert_eq!(fs::read_dir(data.join("Trash/files")).unwrap().count(), 0);
        }
        "partial_unsafe_trash" => {
            let id = partial_receive(&mut h, &source, &dest, &work);
            fs::set_permissions(data.join("Trash"), fs::Permissions::from_mode(0o777)).unwrap();
            let result = h.run(Command::Undo(id));
            assert!(matches!(result.outcome, Outcome::Failed(_)), "{result:?}");
            assert_eq!(fs::read(&source).unwrap(), b"original");
            assert_eq!(fs::read(dest.join("source")).unwrap(), b"original");
            assert_eq!(fs::read_dir(data.join("Trash/files")).unwrap().count(), 0);
            fs::set_permissions(data.join("Trash"), fs::Permissions::from_mode(0o700)).unwrap();
        }
        "ordinary_partial_move" => {
            let a = work.join("one");
            let b = work.join("two");
            fs::create_dir(&a).unwrap();
            fs::create_dir(&b).unwrap();
            let first = a.join("same-name");
            let second = b.join("same-name");
            fs::write(&first, b"first original").unwrap();
            fs::write(&second, b"second original").unwrap();
            // Оба назначения свободны на стадии prepare, конфликт возникает после первого rename.
            let result = h.run(Command::Move(
                vec![first.clone(), second.clone()],
                dest.clone(),
            ));
            assert!(matches!(result.outcome, Outcome::Failed(_)));
            assert_eq!((result.completed, result.remaining), (Some(1), Some(1)));
            let id = h.history.last().unwrap().id;
            let (record, checkpoint) = saved_checkpoint(&root, id);
            assert_eq!(record["finished"]["result"], "Partial");
            assert_eq!(checkpoint["receive_move"], false);
            assert!(!first.exists());
            assert_eq!(fs::read(dest.join("same-name")).unwrap(), b"first original");
            h.success(Command::Undo(id));
            assert_eq!(fs::read(first).unwrap(), b"first original");
            assert_eq!(fs::read(second).unwrap(), b"second original");
            assert!(!dest.join("same-name").exists());
        }
        "legacy_checkpoint" => {
            let id = partial_receive(&mut h, &source, &dest, &work);
            let (_, mut checkpoint) = saved_checkpoint(&root, id);
            assert_eq!(
                checkpoint.as_object_mut().unwrap().remove("receive_move"),
                Some(serde_json::Value::Bool(true))
            );
            drop(h);
            let mut journal =
                history::Journal::open(&root.join("state/papka"), files::now()).unwrap();
            journal
                .update_record(id, serde_json::to_vec(&checkpoint).unwrap())
                .unwrap();
            drop(journal);
            let mut reopened = Harness::new();
            let result = reopened.run(Command::Undo(id));
            assert!(
                matches!(result.outcome,Outcome::Failed(ref error) if error.contains("original name is in use")),
                "{result:?}"
            );
            assert_eq!(fs::read(&source).unwrap(), b"original");
            assert_eq!(fs::read(dest.join("source")).unwrap(), b"original");
            assert!(
                reopened
                    .history
                    .iter()
                    .find(|record| record.id == id)
                    .unwrap()
                    .undo_available,
                "Старый checkpoint успешно десериализован"
            );
        }
        "missing_source_parent" => {
            let parent = work.join("sender-parent");
            fs::create_dir(&parent).unwrap();
            let original = parent.join("source");
            fs::write(&original, b"original").unwrap();
            let id = partial_receive(&mut h, &original, &dest, &work);
            fs::remove_file(&original).unwrap();
            fs::remove_dir(&parent).unwrap();
            h.success(Command::Undo(id));
            assert!(!parent.exists());
            assert!(!dest.join("source").exists());
            assert_eq!(fs::read_dir(data.join("Trash/files")).unwrap().count(), 1);
        }
        other => panic!("Неизвестный сценарий: {other}"),
    }
}
