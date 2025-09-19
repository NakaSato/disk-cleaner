use crate::scanner;
use crossterm::event::{KeyCode, KeyEvent};
use glob::Pattern;
use ratatui::widgets::ListState;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{SystemTime, UNIX_EPOCH},
};
use walkdir::WalkDir;

// App state enum
#[derive(PartialEq, Eq)]
pub enum AppState {
    Scanning,
    Stopping,
    ScanComplete,
    DeletionComplete,
}

// Messages from scan thread
pub enum ScanUpdate {
    Path(PathBuf),
    Result(DirInfo),
    Done,
}

// Struct to represent directory information
#[derive(Debug, Clone)]
pub struct DirInfo {
    pub path: PathBuf,
    pub modified_days_ago: u32,
    pub selected: bool,
    pub size_bytes: u64,
}

// Struct to hold scan results
#[derive(Debug, Clone, Default)]
pub struct ScanResults {
    pub total_folders: usize,
    pub found_folders: usize,
    pub total_size_gb: f64,
    pub selected_size_gb: f64,
}

// App state
pub struct App {
    pub state: AppState,
    pub spinner_index: usize,
    pub current_scan_path: Option<PathBuf>,
    pub scan_receiver: Option<mpsc::Receiver<ScanUpdate>>,
    pub scan_stop_signal: Arc<AtomicBool>,
    pub deletion_summary: Option<(usize, u64)>,
    pub folders_to_clean: Vec<String>,
    pub selected_folders: Vec<bool>,
    pub ignore_patterns: Vec<String>,
    pub current_directory: PathBuf,
    pub dirs_to_clean: Vec<DirInfo>,
    pub dir_list_state: ListState,
    pub confirm_action: Option<String>,
    pub scan_results: ScanResults,
    pub should_exit: bool,
}

impl App {
    pub fn new() -> Self {
        App {
            state: AppState::Scanning,
            spinner_index: 0,
            current_scan_path: None,
            scan_receiver: None,
            scan_stop_signal: Arc::new(AtomicBool::new(false)),
            deletion_summary: None,
            folders_to_clean: vec!["node_modules".to_string(), "target".to_string()],
            selected_folders: vec![true, true],
            ignore_patterns: vec![".*".to_string()],
            current_directory: PathBuf::from("."),
            dirs_to_clean: Vec::new(),
            dir_list_state: ListState::default(),
            confirm_action: None,
            scan_results: ScanResults::default(),
            should_exit: false,
        }
    }

    pub fn start_scan(&mut self) {
        let (tx, rx) = mpsc::channel();
        self.scan_receiver = Some(rx);
        self.state = AppState::Scanning;
        self.dirs_to_clean.clear(); // Clear previous results
        self.scan_stop_signal.store(false, Ordering::SeqCst);

        let stop_signal = self.scan_stop_signal.clone();
        let current_directory = self.current_directory.clone();
        let folders_to_clean = self.folders_to_clean.clone();
        let ignore_patterns = self.ignore_patterns.clone();

        thread::spawn(move || {
            let ignore_patterns: Vec<Pattern> = ignore_patterns
                .iter()
                .map(|p| Pattern::new(p).expect("Failed to compile glob pattern"))
                .collect();
            let mut it = WalkDir::new(&current_directory).into_iter();

            loop {
                if stop_signal.load(Ordering::SeqCst) {
                    break;
                }
                let entry = match it.next() {
                    Some(Ok(entry)) => entry,
                    Some(Err(_)) => continue, // or handle error
                    None => break,
                };

                let path = entry.path();
                if entry.file_type().is_dir() {
                    let _ = tx.send(ScanUpdate::Path(path.to_path_buf()));

                    // Check against ignore patterns
                    let filename = path.file_name().unwrap_or_default().to_string_lossy();
                    let should_ignore = ignore_patterns.iter().any(|p| p.matches(&filename));

                    if should_ignore {
                        it.skip_current_dir();
                        continue;
                    }
                }

                let is_dir = entry.file_type().is_dir();
                let dir_name = entry.file_name().to_string_lossy();

                if is_dir && folders_to_clean.contains(&dir_name.to_string()) {
                    if let Ok(metadata) = entry.metadata() {
                        let modified_time = match metadata.modified() {
                            Ok(t) => t,
                            Err(_) => UNIX_EPOCH,
                        }
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs();

                        let days_ago = (SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs()
                            - modified_time)
                            / (24 * 60 * 60);

                        let dir_size = scanner::calculate_directory_size(&path.to_path_buf());

                        let dir_info = DirInfo {
                            path: path.to_path_buf(),
                            modified_days_ago: days_ago as u32,
                            selected: days_ago > 30, // Auto-select directories older than 30 days
                            size_bytes: dir_size,
                        };
                        let _ = tx.send(ScanUpdate::Result(dir_info));
                    }
                    it.skip_current_dir();
                }
            }
            let _ = tx.send(ScanUpdate::Done);
        });
    }

    pub fn move_dirs_to_trash(&self) -> (usize, u64) {
        let mut deleted_count = 0;
        let mut deleted_size = 0;

        for dir in &self.dirs_to_clean {
            if dir.selected && trash::delete(&dir.path).is_ok() {
                deleted_count += 1;
                deleted_size += dir.size_bytes;
            }
        }
        (deleted_count, deleted_size)
    }

    pub fn update_selection_scan_results(&mut self) {
        let (count, size) = self
            .dirs_to_clean
            .iter()
            .filter(|d| d.selected)
            .fold((0, 0), |(count, size), dir| {
                (count + 1, size + dir.size_bytes)
            });
        self.scan_results.found_folders = count;
        self.scan_results.selected_size_gb = size as f64 / (1024.0 * 1024.0 * 1024.0);
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) {
        if let AppState::DeletionComplete = self.state {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => self.should_exit = true,
                _ => {}
            }
            return;
        }

        if let Some(ref action) = self.confirm_action.clone() {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    if action.starts_with("Move") {
                        let (count, size) = self.move_dirs_to_trash();
                        self.deletion_summary = Some((count, size));
                        self.state = AppState::DeletionComplete;
                    } else if action == "Stop the current scan" {
                        self.scan_stop_signal.store(true, Ordering::SeqCst);
                        self.state = AppState::Stopping;
                    }
                    self.confirm_action = None;
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.confirm_action = None;
                }
                _ => {}
            }
            return;
        }

        match self.state {
            AppState::Scanning => match key.code {
                KeyCode::Char('q') => self.should_exit = true,
                KeyCode::Esc => {
                    self.confirm_action = Some("Stop the current scan".to_string());
                }
                _ => {}
            },
            AppState::Stopping => {
                // Ignore key events while stopping
            }
            AppState::ScanComplete | AppState::DeletionComplete => match key.code {
                KeyCode::Char('q') | KeyCode::Esc => self.should_exit = true,
                // Handle list navigation with clamped indices
                KeyCode::Down => {
                    // Handle list navigation down with proper bounds checking
                    if !self.dirs_to_clean.is_empty() {
                        let current_selection = self.dir_list_state.selected().unwrap_or(0);
                        // Make sure we don't go beyond the list length
                        if current_selection + 1 < self.dirs_to_clean.len() {
                            self.dir_list_state.select(Some(current_selection + 1));
                        }
                    }
                }
                // Handle list navigation with clamped indices
                KeyCode::Up => {
                    // Handle list navigation up with proper bounds checking
                    if !self.dirs_to_clean.is_empty() {
                        let current_selection = self.dir_list_state.selected().unwrap_or(0);
                        // Make sure we don't go below 0
                        if current_selection > 0 {
                            self.dir_list_state.select(Some(current_selection - 1));
                        }
                    }
                }
                KeyCode::Enter => {
                    if !self.dirs_to_clean.is_empty() {
                        // Proceed to confirmation when Enter is pressed in list
                        let selected_count =
                            self.dirs_to_clean.iter().filter(|d| d.selected).count();
                        if selected_count > 0 {
                            self.confirm_action =
                                Some(format!("Move {} selected items to trash", selected_count));
                        }
                    }
                }
                KeyCode::Char(' ') => {
                    // Toggle selection of current directory
                    if !self.dirs_to_clean.is_empty() {
                        if let Some(selected) = self.dir_list_state.selected() {
                            if selected < self.dirs_to_clean.len() {
                                self.dirs_to_clean[selected].selected =
                                    !self.dirs_to_clean[selected].selected;
                            }
                        }
                    }
                    self.update_selection_scan_results();
                }
                KeyCode::Char('a') => {
                    // Select all directories
                    for dir in &mut self.dirs_to_clean {
                        dir.selected = true;
                    }
                    self.update_selection_scan_results();
                }
                KeyCode::Char('d') => {
                    // Deselect all directories
                    for dir in &mut self.dirs_to_clean {
                        dir.selected = false;
                    }
                    self.update_selection_scan_results();
                }
                KeyCode::Char('c') => {
                    // Confirm deletion
                    if !self.dirs_to_clean.is_empty() {
                        let selected_count =
                            self.dirs_to_clean.iter().filter(|d| d.selected).count();
                        if selected_count > 0 {
                            self.confirm_action =
                                Some(format!("Move {} selected items to trash", selected_count));
                        }
                    }
                }
                _ => {}
            },
        }
    }
}
