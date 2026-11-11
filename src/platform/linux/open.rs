//! Открытие файлов стандартным приложением Linux.
pub fn open_file(path: &std::path::Path) -> std::io::Result<()> {
    // Абсолютный путь исключает интерпретацию имени как аргумента командной строки.
    let status = std::process::Command::new("xdg-open")
        .arg(path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(
            "Could not open the file with the default application.",
        ))
    }
}
