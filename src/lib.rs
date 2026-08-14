#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
compile_error!("Lisle supports only x86_64-linux");

pub mod composition;
pub mod engine;
pub mod ibus;
pub mod key;
