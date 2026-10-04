//! Clockify TUI library.

pub mod config;
pub use config::AppConfig;

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    println!("Hello from clockify-tui!");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stub_run() {
        assert!(run().is_ok());
    }
}
