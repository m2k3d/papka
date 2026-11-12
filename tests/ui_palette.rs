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
    // Проверяем реальные обработчики оформления, а не сохранение сортировки / hidden.
    // Без Background этот тест не читает каталог и не пишет настройки.
    state.set_dark(true);
    state.set_palette_color(1, "AABBCC".into());
    state.set_palette_count(2);
    state.cycle_palette_combination();
    state.set_palette(Settings::DEFAULT_PALETTE);
    assert_eq!(state.directory, directory);
    assert_eq!(
        state.page, 2,
        "Изменение оформления не должно возвращать к первой странице файлов"
    );
}

fn angle_radius(point: [f32; 2]) -> (f32, f32) {
    let (x, y) = (point[0] - 0.5, point[1] - 0.5);
    (y.atan2(x).to_degrees().rem_euclid(360.0), x.hypot(y))
}
fn assert_harmony(settings: &Settings, offsets: &[f32]) {
    let (base, radius) = angle_radius(settings.wheel_position(0));
    for (index, expected) in offsets.iter().enumerate() {
        let (angle, r) = angle_radius(settings.wheel_position(index));
        assert!((r - radius).abs() < 0.00001, "радиусы: {r} / {radius}");
        if radius > 0.0001 {
            let error = (angle - base - expected + 180.).rem_euclid(360.) - 180.;
            assert!(
                error.abs() < 0.002,
                "угол {index}: {angle}, база {base}, offset {expected}"
            );
        }
    }
}

#[test]
fn harmony_angles_and_shared_radius_survive_dragging_each_point() {
    use files::PaletteHarmony::*;
    for (scheme, offsets) in [
        (Complementary, vec![0., 180.]),
        (SingleAnalogous, vec![0., 310.]),
        (SplitComplementary, vec![0., 150., 210.]),
        (Analogous, vec![0., 50., 310.]),
        (Triadic, vec![0., 120., 240.]),
    ] {
        let mut settings = Settings {
            palette_count: offsets.len() as u8,
            ..Settings::default()
        };
        settings.apply_palette_harmony(scheme);
        for index in 0..offsets.len() {
            // Переход через 0°, внутренность, центр и выход за окружность.
            for point in [
                [0.8, 0.499],
                [0.8, 0.501],
                [0.6, 0.3],
                [0.5, 0.5],
                [-1., 2.],
            ] {
                settings.move_palette_point(index, point);
                assert_eq!(settings.harmony(), scheme);
                assert_harmony(&settings, &offsets);
                let expected = files::circle_position(point);
                let actual = settings.wheel_position(index);
                for axis in 0..2 {
                    assert!((expected[axis] - actual[axis]).abs() < 0.00001);
                }
                assert!(settings.palette.iter().all(|&c| c <= 0xffffff));
            }
        }
    }
}

#[test]
fn custom_drag_preserves_other_colors_and_wheel_positions() {
    let mut settings = Settings::default();
    settings.apply_palette_harmony(files::PaletteHarmony::Custom);
    let before = settings.clone();
    settings.move_palette_point(1, [0.8, 0.5]);
    assert_ne!(settings.palette[1], before.palette[1]);
    for index in [0, 2] {
        assert_eq!(settings.palette[index], before.palette[index]);
        assert_eq!(settings.wheel_position(index), before.wheel_position(index));
    }
    assert_eq!(settings.harmony(), files::PaletteHarmony::Custom);
}

#[test]
fn legacy_palette_stays_exact_until_edit_and_new_harmonies_round_trip() {
    let mut legacy: Settings = serde_json::from_str(r#"{"palette":[1193046,11259375,16777215],"palette_count":3,"palette_positions":[[0.1,0.9],[0.8,0.3],[0.4,0.7]],"palette_combination":2}"#).unwrap();
    assert_eq!(legacy.palette_harmony, None);
    assert_eq!(legacy.harmony(), files::PaletteHarmony::Custom);
    assert_eq!(legacy.palette, [0x123456, 0xabcdef, 0xffffff]);
    let colors = legacy.palette;
    let positions = legacy.palette_positions;
    for _ in 0..2 {
        let restored: Settings =
            serde_json::from_slice(&serde_json::to_vec(&legacy).unwrap()).unwrap();
        assert_eq!(restored.palette, colors);
        assert_eq!(restored.palette_positions, positions);
        assert_eq!(restored.palette_harmony, None);
        legacy = restored;
    }
    legacy.prepare_palette_wheel();
    assert_eq!(legacy.palette, colors);
    assert_eq!(legacy.harmony(), files::PaletteHarmony::Custom);
    for count in 1..=3 {
        for &scheme in files::PaletteHarmony::choices(count) {
            legacy.palette_count = count;
            legacy.apply_palette_harmony(scheme);
            let restored: Settings =
                serde_json::from_slice(&serde_json::to_vec(&legacy).unwrap()).unwrap();
            assert_eq!(restored.palette, legacy.palette);
            assert_eq!(restored.palette_positions, legacy.palette_positions);
            assert_eq!(restored.palette_harmony, Some(scheme));
        }
    }
}

#[test]
fn hex_exactness_achromatic_colors_and_linked_hues() {
    let mut settings = Settings::default();
    for index in 0..3 {
        for color in [0x12abef, 0xffffff, 0x000000, 0x808080] {
            settings.apply_palette_color(index, color);
            assert_eq!(settings.palette[index], color);
            assert_harmony(&settings, &[0., 120., 240.]);
            if color == 0 || color == 0xffffff || color == 0x808080 {
                assert!(settings.palette.iter().all(|&value| value == color));
            }
        }
    }
}

#[test]
fn count_changes_preserve_analogous_family_and_default_reset_is_complete() {
    use files::PaletteHarmony::*;
    let mut state = app::App::default();
    state.settings.apply_palette_harmony(Analogous);
    state.set_palette_count(2);
    assert_eq!(state.settings.harmony(), SingleAnalogous);
    assert_harmony(&state.settings, &[0., 310.]);
    state.set_palette_count(3);
    assert_eq!(state.settings.harmony(), Analogous);
    assert_harmony(&state.settings, &[0., 50., 310.]);
    for count in [1, 2, 3, 0, 255] {
        state.set_palette_count(count);
        assert_eq!(state.settings.palette_count, count.clamp(1, 3));
        assert!(files::PaletteHarmony::choices(count).contains(&state.settings.harmony()));
    }
    state.settings.dark = true;
    state.settings.hidden = true;
    state.settings.palette_intensity = 0.87;
    state.palette_active = 2;
    state.settings.move_palette_point(2, [0.1, 0.6]);
    state.set_palette(Settings::DEFAULT_PALETTE);
    let defaults = Settings::default();
    assert_eq!(state.settings.palette, defaults.palette);
    assert_eq!(state.settings.palette_count, defaults.palette_count);
    for i in 0..3 {
        for axis in 0..2 {
            assert!(
                (state.settings.palette_positions[i][axis] - defaults.palette_positions[i][axis])
                    .abs()
                    < 0.00001
            );
        }
    }
    assert_eq!(state.settings.palette_intensity, defaults.palette_intensity);
    assert_eq!(state.settings.palette_harmony, defaults.palette_harmony);
    assert_eq!(state.palette_active, 0);
    assert_eq!(
        state.palette_inputs,
        defaults.palette.map(|c| format!("{c:06X}"))
    );
    assert!(state.settings.dark && state.settings.hidden);
}

#[test]
fn hsl_primary_colors_black_and_white_match_rgb() {
    for (hue, expected) in [
        (0., 0xff0000),
        (60., 0xffff00),
        (120., 0x00ff00),
        (180., 0x00ffff),
        (240., 0x0000ff),
        (300., 0xff00ff),
        (360., 0xff0000),
    ] {
        assert_eq!(files::hsl_color(hue, 1., 0.5), expected);
        assert_eq!(files::hsl_color(hue, 1., 0.), 0);
        assert_eq!(files::hsl_color(hue, 1., 1.), 0xffffff);
    }
}

#[test]
fn dragging_and_hex_change_linked_rgb_colors_with_the_wheel() {
    let mut settings = Settings::default();
    settings.move_palette_point(0, [0.75, 0.5]);
    assert_eq!(settings.palette, [0xf90606, 0x06f906, 0x0606f9]);
    settings.move_palette_point(1, [0.75, 0.5]);
    assert_eq!(settings.palette, [0x0606f9, 0xf90606, 0x06f906]);
    settings.apply_palette_color(0, 0xff0000);
    assert_eq!(settings.palette, [0xff0000, 0x00ff00, 0x0000ff]);
    settings.palette_count = 2;
    settings.apply_palette_harmony(files::PaletteHarmony::Complementary);
    assert_eq!(settings.palette[..2], [0xff0000, 0x00ffff]);
}
