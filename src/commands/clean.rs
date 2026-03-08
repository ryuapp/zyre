use std::path::Path;

pub fn run() {
    let mut file_count = 0;
    let mut total_bytes = 0;
    for dir in &["zyre-out", "zyre-cache"] {
        let p = Path::new(dir);
        let (c, b) = walk_dir_stats(p);
        file_count += c;
        total_bytes += b;
        match std::fs::remove_dir_all(p) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => panic!("Failed to remove {}: {}", dir, e),
        }
    }
    eprintln!(
        "Removed {} files, {}",
        file_count,
        format_bytes(total_bytes)
    );
}

fn walk_dir_stats(dir: &Path) -> (usize, u64) {
    let mut count = 0;
    let mut bytes = 0u64;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let (c, b) = walk_dir_stats(&path);
                count += c;
                bytes += b;
            } else {
                count += 1;
                bytes += entry.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }
    }
    (count, bytes)
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.1}GB total", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1}MB total", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1}KB total", bytes as f64 / KB as f64)
    } else {
        format!("{}B total", bytes)
    }
}
