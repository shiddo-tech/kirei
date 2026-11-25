use std::io::Write;
use std::path::PathBuf;

/// Local crash reports; nothing leaves the Mac.
pub fn folder() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join("Library/Logs/Kirei")
}

/// Writes panics to `~/Library/Logs/Kirei`, like the Swift app keeps MetricKit reports there.
pub fn start() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let dir = folder();
        if std::fs::create_dir_all(&dir).is_ok() {
            let t = crate::platform::local_time();
            let name = format!(
                "crash-{:04}{:02}{:02}-{:02}{:02}{:02}.txt",
                t.year, t.month, t.day, t.hour, t.minute, t.second
            );
            if let Ok(mut f) = std::fs::File::create(dir.join(name)) {
                let bt = std::backtrace::Backtrace::force_capture();
                writeln!(f, "Kirei {}\n{info}\n\n{bt}", env!("CARGO_PKG_VERSION")).ok();
            }
        }
        previous(info);
    }));
}
