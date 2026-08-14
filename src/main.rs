use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    if !std::env::args()
        .skip(1)
        .all(|argument| argument == "--ibus")
    {
        eprintln!("lisle: only --ibus is supported");
        return ExitCode::FAILURE;
    }

    match lisle::ibus::run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("lisle: {error}");
            ExitCode::FAILURE
        }
    }
}
