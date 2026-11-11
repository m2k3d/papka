// Подключается к временной копии ui.rs; реализация приложения не меняется.
#[cfg(test)]
mod app_state_tests {
    use crate::app::{self, App, Dialog};
    use crate::files::{Command, Data};
    use crate::operations::{Background, Completion, Event, Outcome};
    use std::{path::Path, sync::mpsc, time::Duration};

    fn success() -> Completion {
        app::completion(Ok(()))
    }

    // Явная доставка событий с ограничением ожидания вместо задержек по времени.
    fn background() -> (Background, mpsc::Receiver<Event>) {
        let (background, events) = Background::start().unwrap();
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            while let Ok(event) = events.recv_blocking() {
                if sender.send(event).is_err() {
                    break;
                }
            }
        });
        (background, receiver)
    }

    fn finish(app: &mut App, receiver: &mpsc::Receiver<Event>, id: u64) {
        loop {
            let event = receiver.recv_timeout(Duration::from_secs(5)).unwrap();
            let done = matches!(&event, Event::Finished { id: finished, .. } if *finished == id);
            app.apply(event);
            if done {
                break;
            }
        }
    }

    fn fixture(path: &Path) -> (App, mpsc::Receiver<Event>) {
        let (background, receiver) = background();
        let mut app = App::default();
        app.directory = path.into();
        app.background = Some(background);
        app.refresh();
        let id = app.loading.unwrap();
        finish(&mut app, &receiver, id);
        assert!(app.error.is_none());
        (app, receiver)
    }

    #[test]
    fn listing_batches_commit_atomically_and_ignore_stale_ids() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["a", "b"] {
            std::fs::write(dir.path().join(name), b"fixture").unwrap();
        }
        let (mut app, _) = fixture(dir.path());
        let entries = app.entries.clone();
        app.background.take().unwrap().shutdown();
        app.loading = Some(80);
        app.apply(Event::Data {
            id: 79,
            data: Data::ListingBegin(dir.path().join("stale")),
        });
        app.apply(Event::Finished {
            id: 79,
            completion: app::completion(Err(std::io::Error::other("stale"))),
        });
        assert!(app.error.is_none());
        assert_eq!(app.loading, Some(80));
        app.apply(Event::Data {
            id: 80,
            data: Data::ListingBegin(dir.path().to_path_buf()),
        });
        app.apply(Event::Data {
            id: 80,
            data: Data::ListingBatch(vec![entries[0].clone()]),
        });
        app.apply(Event::Data {
            id: 79,
            data: Data::ListingBatch(entries.clone()),
        });
        assert_eq!(
            app.entries.len(),
            2,
            "Неполный список не должен появляться до ListingEnd"
        );
        app.apply(Event::Data {
            id: 80,
            data: Data::ListingBatch(vec![entries[1].clone()]),
        });
        app.apply(Event::Data {
            id: 79,
            data: Data::ListingEnd(None),
        });
        app.apply(Event::Data {
            id: 80,
            data: Data::ListingEnd(Some("warning".into())),
        });
        assert_eq!(
            app.entries
                .iter()
                .map(|e| e.name.as_str())
                .collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert_eq!(app.status, "warning");
        app.apply(Event::Finished {
            id: 80,
            completion: success(),
        });
        app.loading = Some(81);
        // Порция без начала нового списка не должна использовать прежний буфер.
        app.apply(Event::Data {
            id: 81,
            data: Data::ListingBatch(entries),
        });
        app.apply(Event::Data {
            id: 81,
            data: Data::ListingEnd(None),
        });
        assert_eq!(app.status, "warning");
    }

    #[test]
    fn history_changes_only_after_successful_read_and_new_navigation_clears_forward() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        let c = dir.path().join("c");
        for path in [&a, &b, &c] {
            std::fs::create_dir(path).unwrap();
        }
        let (mut app, receiver) = fixture(dir.path());
        for path in [&a, &b] {
            app.navigate(path.clone());
            let id = app.loading.unwrap();
            finish(&mut app, &receiver, id);
        }
        std::fs::remove_dir(&a).unwrap();
        app.back();
        let id = app.loading.unwrap();
        finish(&mut app, &receiver, id);
        assert_eq!(app.directory, b);
        assert!(app.can_back());
        assert!(!app.can_forward());
        assert!(app.error.is_some());
        std::fs::create_dir(&a).unwrap();
        app.back();
        let id = app.loading.unwrap();
        finish(&mut app, &receiver, id);
        assert_eq!(app.directory, a);
        assert!(app.error.is_none());
        assert!(app.can_forward());
        app.forward();
        let id = app.loading.unwrap();
        finish(&mut app, &receiver, id);
        assert_eq!(app.directory, b);
        app.back();
        let id = app.loading.unwrap();
        finish(&mut app, &receiver, id);
        app.navigate(c.clone());
        let id = app.loading.unwrap();
        finish(&mut app, &receiver, id);
        assert_eq!(app.directory, c);
        assert!(!app.can_forward());
    }

    #[test]
    fn selection_methods_preserve_anchor_group_and_double_click_navigation() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("folder");
        std::fs::create_dir(&folder).unwrap();
        for name in ["a", "b", "c"] {
            std::fs::write(dir.path().join(name), b"fixture").unwrap();
        }
        let (mut app, receiver) = fixture(dir.path());
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        let c = dir.path().join("c");
        app.select(a.clone());
        app.selection_modifiers(false, true);
        app.select(c.clone());
        assert_eq!(app.selected(), vec![a.clone(), b.clone(), c.clone()]);
        app.selection_modifiers(true, false);
        app.select(b.clone());
        assert_eq!(app.selected(), vec![a.clone(), c.clone()]);
        app.select_context(a);
        assert_eq!(app.selection.len(), 2);
        app.copy_selection(true);
        assert_eq!(app.clipboard.as_ref().unwrap().0.len(), 2);
        assert!(app.clipboard.as_ref().unwrap().1);
        app.select_all();
        assert_eq!(app.selection.len(), 4);
        app.selection_modifiers(false, false);
        app.select(folder.clone());
        app.select(folder.clone());
        let id = app
            .loading
            .expect("Двойной щелчок должен запустить чтение папки");
        finish(&mut app, &receiver, id);
        assert_eq!(app.directory, folder);
        assert!(app.selection.is_empty());
    }

    #[test]
    fn dialog_methods_reject_invalid_names_and_respect_conflict_and_busy_state() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = App::default();
        app.directory = dir.path().into();
        app.conflict = Some(Command::Create(dir.path().join("occupied"), false));
        app.begin_dialog(Dialog::Create(true));
        assert!(app.dialog.is_none());
        app.clear_error();
        app.begin_dialog(Dialog::Create(false));
        for value in ["", ".", "..", "a/b", "a\0b"] {
            app.set_input(value.into());
            app.confirm();
            assert!(app.dialog.is_some());
            assert!(app.error.is_some());
        }
        app.dismiss();
        let (background, _) = background();
        let ticket = background.submit(|_| success()).unwrap();
        app.mutation = Some(ticket);
        app.begin_dialog(Dialog::Transfer(true));
        assert!(app.dialog.is_none());
        background.shutdown();
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn stop_close_and_partial_completion_preserve_operation_notice() {
        let (background, _) = background();
        let ticket = background.submit(|_| success()).unwrap();
        let id = ticket.id;
        let cancellation = ticket.cancellation.clone();
        let mut app = App::default();
        app.mutation = Some(ticket);
        assert!(!app.can_close());
        app.apply(Event::Finished {
            id: id + 1,
            completion: success(),
        });
        assert!(app.mutation.is_some());
        app.stop();
        assert!(cancellation.is_requested());
        app.request_close();
        assert!(app.closing);
        assert!(!app.can_close());
        app.apply(Event::Finished {
            id,
            completion: Completion {
                outcome: Outcome::Interrupted,
                completed: Some(1),
                remaining: Some(2),
                undo_available: true,
            },
        });
        assert!(app.can_close());
        assert!(app.status.contains("Обработано: 1; осталось: 2"));
        assert!(app.status.contains("Прерывание"));
        assert_eq!(app.operation_notice.as_deref(), Some(app.status.as_str()));
        background.shutdown();
    }

    #[test]
    fn storage_commands_run_in_isolated_child() {
        let dir = tempfile::tempdir().unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "ui::app_state_tests::isolated_storage_probe",
                "--ignored",
                "--nocapture",
            ])
            .env("PAPKA_STAGE2_SANDBOX", dir.path())
            .env("XDG_CONFIG_HOME", dir.path().join("config"))
            .env("XDG_DATA_HOME", dir.path().join("data"))
            .env("XDG_STATE_HOME", dir.path().join("state"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    #[ignore = "Вызывается только родительским тестом с изолированными XDG-каталогами"]
    fn isolated_storage_probe() {
        let root = std::path::PathBuf::from(std::env::var_os("PAPKA_STAGE2_SANDBOX").unwrap());
        let dirs = crate::platform::AppDirectories::resolve().unwrap();
        assert!(dirs.config.starts_with(&root));
        assert!(dirs.history.starts_with(&root));
        let source = root.join("source");
        let destination = root.join("destination");
        std::fs::create_dir(&source).unwrap();
        std::fs::create_dir(&destination).unwrap();
        std::fs::write(source.join("original"), b"original").unwrap();
        let (mut app, receiver) = fixture(&source);
        app.persist_settings();
        assert!(
            !app.can_close(),
            "Необработанное auxiliary-сохранение удерживает закрытие"
        );
        app.request_close();
        while !app.can_close() {
            app.apply(receiver.recv_timeout(Duration::from_secs(5)).unwrap());
        }
        assert!(app.mutation.is_none());
        assert!(app.closing);
        assert!(dirs.config.join("settings.json").is_file());
        app.closing = false;
        app.conflict = Some(Command::Copy(
            vec![source.join("original")],
            destination.clone(),
        ));
        app.resolve_conflict();
        assert!(matches!(app.dialog, Some(Dialog::Transfer(false))));
        app.selection.clear();
        app.selection
            .insert(source.join("not-the-original-selection"));
        app.set_input(destination.to_string_lossy().into_owned());
        app.confirm();
        let id = app.mutation.as_ref().unwrap().id;
        finish(&mut app, &receiver, id);
        assert!(app.error.is_none(), "{:?}", app.error);
        assert_eq!(
            std::fs::read(destination.join("original")).unwrap(),
            b"original"
        );
        assert!(!destination.join("not-the-original-selection").exists());
        // Повтор создания сохраняет исходную папку конфликта, даже если открыта другая.
        app.conflict = Some(Command::Create(destination.join("occupied"), false));
        app.resolve_conflict();
        app.set_input("new-name".into());
        app.confirm();
        let id = app.mutation.as_ref().unwrap().id;
        finish(&mut app, &receiver, id);
        assert!(destination.join("new-name").is_file());
        assert!(!source.join("new-name").exists());
    }
}
