#[path = "../src/history.rs"]
#[allow(dead_code)]
mod history;

use history::{Action, Journal, ResultKind, HISTORY_AGE_SECONDS, HISTORY_LIMIT};
use serde_json::{json, Value};
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

fn record(id: u64, finished: Option<u64>) -> Value {
    json!({"id": id, "action": "Copy", "started_at": 10,
        "finished": finished.map(|at| json!({"at":at,"result":"Success"})),
        "checkpoint": [0,255,1]})
}

fn fixture(path: &Path, records: Vec<Value>, next_id: u64) -> Vec<u8> {
    let bytes = serde_json::to_vec(&json!({"version":1,"next_id":next_id,"records":records})).unwrap();
    fs::write(path.join("history.json"), &bytes).unwrap();
    bytes
}

#[test]
fn checkpoints_and_results_survive_reopening() {
    let directory = tempfile::tempdir().unwrap();
    let mut journal = Journal::open(directory.path(), 100).unwrap();
    let first = journal.begin(Action::Copy, vec![0, 255], 100).unwrap();
    journal.checkpoint(first, vec![255, 0, 128]).unwrap();
    drop(journal);
    let mut journal = Journal::open(directory.path(), 101).unwrap();
    assert!(journal.records()[0].finished.is_none());
    assert_eq!(journal.records()[0].checkpoint, [255, 0, 128]);
    journal.finish(first, ResultKind::Partial, vec![1, 255], 102).unwrap();
    drop(journal);
    let mut journal = Journal::open(directory.path(), 103).unwrap();
    let saved = &journal.records()[0];
    assert_eq!(saved.id, first);
    assert!(matches!(saved.action, Action::Copy));
    assert!(matches!(saved.finished.as_ref().unwrap().result, ResultKind::Partial));
    assert_eq!(saved.finished.as_ref().unwrap().at, 102);
    assert_eq!(saved.checkpoint, [1, 255]);
    assert_eq!(journal.begin(Action::Rename, vec![], 103).unwrap(), first + 1);
}

#[test]
fn completed_record_cannot_be_updated_or_finished_twice() {
    let directory = tempfile::tempdir().unwrap();
    let mut journal = Journal::open(directory.path(), 100).unwrap();
    let id = journal.begin(Action::Trash, vec![], 100).unwrap();
    journal.finish(id, ResultKind::Success, vec![1], 100).unwrap();
    let bytes = fs::read(directory.path().join("history.json")).unwrap();
    assert!(journal.checkpoint(id, vec![2]).is_err());
    assert!(journal.finish(id, ResultKind::Success, vec![2], 101).is_err());
    assert!(journal.checkpoint(999, vec![]).is_err());
    assert_eq!(bytes, fs::read(directory.path().join("history.json")).unwrap());
}

#[test]
fn completion_limit_keeps_newest_and_preserves_pending() {
    let directory = tempfile::tempdir().unwrap();
    let mut records: Vec<_> = (1..=102).map(|id| record(id, Some(100 + id))).collect();
    records.push(record(103, None));
    fixture(directory.path(), records, 104);
    let journal = Journal::open(directory.path(), 203).unwrap();
    assert_eq!(journal.records().len(), HISTORY_LIMIT + 1);
    assert!(journal.records().iter().all(|record| record.id >= 3));
    assert!(journal.records().iter().any(|record| record.id == 103 && record.finished.is_none()));
    drop(journal);
    let mut journal = Journal::open(directory.path(), 203).unwrap();
    assert_eq!(journal.begin(Action::Move, vec![], 203).unwrap(), 104);
}

#[test]
fn completing_101st_action_evicts_only_oldest_completed_action() {
    let directory = tempfile::tempdir().unwrap();
    fixture(directory.path(), (1..=100).map(|id| record(id, Some(100))).collect(), 101);
    let mut journal = Journal::open(directory.path(), 100).unwrap();
    let id = journal.begin(Action::Move, vec![], 100).unwrap();
    assert_eq!(journal.records().len(), 101);
    journal.finish(id, ResultKind::Success, vec![], 100).unwrap();
    assert_eq!(journal.records().len(), 100);
    assert!(!journal.records().iter().any(|record| record.id == 1));
    assert!(journal.records().iter().any(|record| record.id == id));
}

#[test]
fn exact_age_boundary_prunes_completed_but_never_pending() {
    let directory = tempfile::tempdir().unwrap();
    fixture(directory.path(), vec![record(1, Some(100)), record(2, Some(101)), record(3, None)], 4);
    let mut journal = Journal::open(directory.path(), 100 + HISTORY_AGE_SECONDS).unwrap();
    let ids: Vec<_> = journal.records().iter().map(|record| record.id).collect();
    assert_eq!(ids, [2, 3]);
    assert_eq!(journal.begin(Action::Copy, vec![], 100 + HISTORY_AGE_SECONDS).unwrap(), 4);
    drop(journal);
    let journal = Journal::open(directory.path(), 101 + HISTORY_AGE_SECONDS).unwrap();
    assert!(journal.records().iter().all(|record| record.finished.is_none()));
}

#[test]
fn begin_and_finish_apply_age_limit_during_session() {
    let directory = tempfile::tempdir().unwrap();
    fixture(directory.path(), vec![record(1, Some(100))], 2);
    let mut journal = Journal::open(directory.path(), 100).unwrap();
    let id = journal.begin(Action::Copy, vec![], 100 + HISTORY_AGE_SECONDS).unwrap();
    assert_eq!(journal.records().len(), 1);
    journal.finish(id, ResultKind::Interrupted, vec![], 100 + HISTORY_AGE_SECONDS).unwrap();
    assert!(matches!(journal.records()[0].finished.as_ref().unwrap().result, ResultKind::Interrupted));
}

#[test]
fn corrupt_and_unknown_documents_are_not_overwritten() {
    let cases = [
        b"{broken".to_vec(),
        serde_json::to_vec(&json!({"version":2,"next_id":1,"records":[]})).unwrap(),
        serde_json::to_vec(&json!({"version":1,"next_id":1,"records":[],"unknown":true})).unwrap(),
        serde_json::to_vec(&json!({"version":1,"next_id":3,"records":[record(1,None),record(1,None)]})).unwrap(),
        serde_json::to_vec(&json!({"version":1,"next_id":1,"records":[record(1,None)]})).unwrap(),
        serde_json::to_vec(&json!({"version":1,"next_id":2,"records":[record(1,Some(9))]})).unwrap(),
    ];
    for bytes in cases {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history.json");
        fs::write(&path, &bytes).unwrap();
        let error = Journal::open(directory.path(), 100).err().expect("Некорректный журнал принят");
        assert_eq!(error.kind(), ErrorKind::InvalidData);
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
}

#[test]
fn second_writer_is_rejected_and_lock_is_released_on_drop() {
    let directory = tempfile::tempdir().unwrap();
    let mut first = Journal::open(directory.path(), 100).unwrap();
    let error = Journal::open(directory.path(), 100).err().expect("Второй писатель принят");
    assert_eq!(error.kind(), ErrorKind::WouldBlock);
    first.begin(Action::Copy, vec![], 100).unwrap();
    // Замена JSON не снимает блокировку отдельного lock-файла.
    assert_eq!(Journal::open(directory.path(), 100).err().unwrap().kind(), ErrorKind::WouldBlock);
    drop(first);
    assert_eq!(Journal::open(directory.path(), 100).unwrap().records().len(), 1);
}

#[test]
fn oversized_checkpoint_does_not_change_memory_disk_or_next_id() {
    let directory = tempfile::tempdir().unwrap();
    let mut journal = Journal::open(directory.path(), 100).unwrap();
    let id = journal.begin(Action::Copy, vec![1], 100).unwrap();
    let before = fs::read(directory.path().join("history.json")).unwrap();
    assert!(journal.checkpoint(id, vec![0; 65537]).is_err());
    assert!(journal.finish(id, ResultKind::Success, vec![0; 65537], 101).is_err());
    assert!(journal.begin(Action::Copy, vec![0; 65537], 101).is_err());
    assert_eq!(fs::read(directory.path().join("history.json")).unwrap(), before);
    assert_eq!(journal.records()[0].checkpoint, [1]);
    assert!(journal.records()[0].finished.is_none());
    assert_eq!(journal.begin(Action::Copy, vec![0; 65536], 101).unwrap(), id + 1);
}

#[test]
fn pending_limit_rejects_65th_without_deleting_recovery_records() {
    let directory = tempfile::tempdir().unwrap();
    let mut journal = Journal::open(directory.path(), 100).unwrap();
    for _ in 0..64 { journal.begin(Action::Move, vec![], 100).unwrap(); }
    let before = fs::read(directory.path().join("history.json")).unwrap();
    assert!(journal.begin(Action::Move, vec![], 100).is_err());
    assert_eq!(journal.records().len(), 64);
    assert_eq!(fs::read(directory.path().join("history.json")).unwrap(), before);
    drop(journal);
    assert_eq!(Journal::open(directory.path(), 100 + HISTORY_AGE_SECONDS * 2).unwrap().records().len(), 64);
}

#[test]
fn oversized_file_is_rejected_without_replacement() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("history.json");
    fs::File::create(&path).unwrap().set_len(16 * 1024 * 1024 + 1).unwrap();
    assert_eq!(Journal::open(directory.path(), 100).err().unwrap().kind(), ErrorKind::InvalidData);
    assert_eq!(fs::metadata(path).unwrap().len(), 16 * 1024 * 1024 + 1);
}

#[test]
fn serialized_size_limit_preserves_previous_file_and_records() {
    let directory = tempfile::tempdir().unwrap();
    let records = (1..=63).map(|id| {
        let mut value = record(id, Some(100));
        value["checkpoint"] = json!(vec![255u8; 65536]);
        value
    }).collect();
    let before = fixture(directory.path(), records, 64);
    assert!(before.len() < 16 * 1024 * 1024);
    let mut journal = Journal::open(directory.path(), 100).unwrap();
    assert!(journal.begin(Action::Copy, vec![255; 65536], 100).is_err());
    assert_eq!(journal.records().len(), 63);
    assert_eq!(fs::read(directory.path().join("history.json")).unwrap(), before);
    assert_eq!(journal.begin(Action::Copy, vec![], 100).unwrap(), 64);
}

#[test]
fn write_error_preserves_previous_file_and_allows_retry() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("journal");
    let moved = root.path().join("moved");
    let mut journal = Journal::open(&path, 100).unwrap();
    let id = journal.begin(Action::Copy, vec![1], 100).unwrap();
    let before = fs::read(path.join("history.json")).unwrap();
    // Ошибка создания временного файла: каталог перемещён только внутри sandbox.
    fs::rename(&path, &moved).unwrap();
    assert!(journal.checkpoint(id, vec![2]).is_err());
    assert_eq!(journal.records()[0].checkpoint, [1]);
    assert_eq!(fs::read(moved.join("history.json")).unwrap(), before);
    fs::rename(&moved, &path).unwrap();
    journal.checkpoint(id, vec![2]).unwrap();
    assert_eq!(journal.records()[0].checkpoint, [2]);
}

#[cfg(unix)]
#[test]
fn newly_created_private_directory_and_files_have_private_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("private");
    let mut journal = Journal::open(&path, 100).unwrap();
    journal.begin(Action::Copy, vec![], 100).unwrap();
    assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o700);
    for name in ["history.json", "history.lock"] {
        assert_eq!(fs::metadata(path.join(name)).unwrap().permissions().mode() & 0o777, 0o600);
    }
}

#[test]
fn replacement_error_keeps_previous_contents_and_cleans_temporary_file() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("journal");
    let mut journal = Journal::open(&path, 100).unwrap();
    let id = journal.begin(Action::Copy, vec![1], 100).unwrap();
    let saved = root.path().join("saved.json");
    let before = fs::read(path.join("history.json")).unwrap();
    fs::rename(path.join("history.json"), &saved).unwrap();
    // Каталог на месте файла вызывает отказ именно атомарной замены.
    fs::create_dir(path.join("history.json")).unwrap();
    assert!(journal.checkpoint(id, vec![2]).is_err());
    assert_eq!(journal.records()[0].checkpoint, [1]);
    assert_eq!(fs::read(&saved).unwrap(), before);
    assert_eq!(fs::read_dir(&path).unwrap().count(), 2);
    fs::remove_dir(path.join("history.json")).unwrap();
    fs::rename(&saved, path.join("history.json")).unwrap();
    journal.checkpoint(id, vec![2]).unwrap();
    drop(journal);
    let journal = Journal::open(&path, 100).unwrap();
    assert_eq!(journal.records()[0].checkpoint, [2]);
}
