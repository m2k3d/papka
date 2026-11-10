mod app;
mod files;
pub mod history;
pub mod operations;
pub mod platform;
mod ui;

fn main() -> std::process::ExitCode {
    match ui::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Не удалось запустить papka: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
