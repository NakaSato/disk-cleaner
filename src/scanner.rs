use std::fs;
use std::path::PathBuf;

pub fn calculate_directory_size(path: &PathBuf) -> u64 {
    let mut total_size = 0u64;

    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata() {
                if metadata.is_dir() {
                    // Recursive call for subdirectories
                    total_size += calculate_directory_size(&entry.path());
                } else {
                    // Add file size
                    total_size += metadata.len();
                }
            }
        }
    }

    total_size
}
