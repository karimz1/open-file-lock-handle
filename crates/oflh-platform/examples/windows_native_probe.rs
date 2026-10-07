//! Developer-only discovery experiments. Results are not end-to-end speedups.
#[cfg(windows)]
#[path = "windows_probe/mod.rs"]
mod probe;

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    probe::run()
}

#[cfg(not(windows))]
fn main() {
    println!("Windows native query experiments require a native Windows runner.");
}
