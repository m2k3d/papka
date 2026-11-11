#![cfg(target_os = "linux")]
#![allow(dead_code)] // Подключены модули приложения, проверяется только оформление.

#[path = "../src/app.rs"]
mod app;
#[path = "../src/files.rs"]
mod files;
#[path = "../src/history.rs"]
mod history;
#[path = "../src/operations.rs"]
mod operations;
#[path = "../src/platform.rs"]
mod platform;

use files::{Settings, Sort};

#[test]
fn legacy_settings_keep_preferences_and_gain_palette() {
    let settings: Settings = serde_json::from_str(
        r#"{
        "hidden": true, "descending": true, "sort": "Modified",
        "dark": true, "reduced_motion": true
    }"#,
    )
    .unwrap();
    assert!(settings.hidden && settings.descending && settings.dark && settings.reduced_motion);
    assert_eq!(settings.sort, Sort::Modified);
    assert_eq!(settings.palette, Settings::default().palette);
    assert_eq!(settings.palette_count, 3);
}

#[test]
fn palette_round_trips_one_two_and_three_colors() {
    for count in 1..=3 {
        let settings = Settings {
            palette: [0x000000, 0x12abef, 0xffffff],
            palette_count: count,
            ..Settings::default()
        };
        let restored: Settings =
            serde_json::from_slice(&serde_json::to_vec(&settings).unwrap()).unwrap();
        assert_eq!(restored.palette, settings.palette);
        assert_eq!(restored.palette_count, count);
    }
}

#[test]
fn malformed_palette_shapes_are_rejected() {
    for json in [
        r#"{"palette":[1,2]}"#,
        r#"{"palette":[1,2,3,4]}"#,
        r#"{"palette":[-1,2,3]}"#,
        r#"{"palette_count":256}"#,
    ] {
        assert!(serde_json::from_str::<Settings>(json).is_err(), "{json}");
    }
}

#[test]
fn settings_persist_in_isolated_child_process() {
    let sandbox = tempfile::tempdir().unwrap();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "settings_storage_probe",
            "--nocapture",
        ])
        .env("XDG_CONFIG_HOME", sandbox.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(std::fs::read_dir(sandbox.path()).unwrap().count(), 1);
}

// Окружение изменяется только у дочернего процесса, пользовательские настройки не затрагиваются.
#[test]
#[ignore]
fn settings_storage_probe() {
    let config =
        std::path::PathBuf::from(std::env::var_os("XDG_CONFIG_HOME").unwrap()).join("papka");
    assert!(!config.exists());
    assert_eq!(
        files::settings_load().unwrap().palette,
        Settings::default().palette
    );
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(
        config.join("settings.json"),
        br#"{"dark":true,"sort":"Size"}"#,
    )
    .unwrap();
    let legacy = files::settings_load().unwrap();
    assert!(legacy.dark);
    assert_eq!(legacy.sort, Sort::Size);
    for count in 1..=3 {
        let settings = Settings {
            palette: [0x123456, 0xabcdef, 0],
            palette_count: count,
            ..legacy.clone()
        };
        files::settings_save(&settings).unwrap();
        let restored = files::settings_load().unwrap();
        assert_eq!(restored.palette, settings.palette);
        assert_eq!(restored.palette_count, count);
        assert!(restored.dark);
        assert_eq!(
            std::fs::read_dir(&config).unwrap().count(),
            1,
            "Временные файлы настроек не должны оставаться"
        );
    }
    std::fs::write(config.join("settings.json"), b"{broken").unwrap();
    assert!(files::settings_load().is_err());
}

#[test]
fn changing_appearance_preserves_current_file_page() {
    let mut state = app::App::default();
    state.page = 2;
    let directory = state.directory.clone();
    state.settings.palette = [0x102030, 0x405060, 0x708090];
    // Так же сохраняют оформление обработчики PaletteColor, PaletteCount и Preset.
    // Без Background этот тест не читает каталог и не пишет настройки.
    state.save_settings();
    assert_eq!(state.directory, directory);
    assert_eq!(
        state.page, 2,
        "Изменение оформления не должно возвращать к первой странице файлов"
    );
}
