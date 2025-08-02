use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

use glob::Pattern;
use std::{
    fs::{self},
    io::stdout,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use walkdir::WalkDir;

const SPINNER_CHARS: [char; 8] = ['⠁', '⠂', '⠄', '⡀', '⢀', '⠠', '⠐', '⠈'];

// App state enum
enum AppState {
    Scanning,
    ScanComplete,
}

// Messages from scan thread
enum ScanUpdate {
    Path(PathBuf),
    Result(DirInfo),
    Done,
}

// Struct to represent directory information
#[derive(Debug, Clone)]
struct DirInfo {
    path: PathBuf,
    modified_days_ago: u32,
    selected: bool,
    size_bytes: u64,
}

// Struct to hold scan results
#[derive(Debug, Clone, Default)]
struct ScanResults {
    total_folders: usize,
    found_folders: usize,
    total_size_gb: f64,
}

// App state
struct App {
    state: AppState,
    spinner_index: usize,
    current_scan_path: Option<PathBuf>,
    scan_receiver: Option<mpsc::Receiver<ScanUpdate>>,
    scan_stop_signal: Arc<AtomicBool>,
    folders_to_clean: Vec<String>,
    selected_folders: Vec<bool>,
    ignore_patterns: Vec<String>,
    current_directory: PathBuf,
    dirs_to_clean: Vec<DirInfo>,
    dir_list_state: ListState,
    confirm_action: Option<String>,
    scan_results: ScanResults,
}

impl App {
    fn new() -> Self {
        App {
            state: AppState::Scanning,
            spinner_index: 0,
            current_scan_path: None,
            scan_receiver: None,
            scan_stop_signal: Arc::new(AtomicBool::new(false)),
            folders_to_clean: vec!["node_modules".to_string(), "target".to_string()],
            selected_folders: vec![true, true],
            ignore_patterns: vec![".*".to_string()],
            current_directory: PathBuf::from("."),
            dirs_to_clean: Vec::new(),
            dir_list_state: ListState::default(),
            confirm_action: None,
            scan_results: ScanResults::default(),
        }
    }

    fn start_scan(&mut self) {
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

                        let dir_size = calculate_directory_size(&path.to_path_buf());

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

    fn move_dirs_to_trash(&self) {
        // Move selected directories to trash
        for dir in &self.dirs_to_clean {
            if dir.selected {
                // Try to move the directory to trash
                match trash::delete(&dir.path) {
                    Ok(()) => println!("Moved to trash: {}", dir.path.display()),
                    Err(e) => eprintln!("Failed to move to trash {}: {}", dir.path.display(), e),
                }
            }
        }
    }

    fn update_selection_scan_results(&mut self) {
        self.scan_results.found_folders = self.dirs_to_clean.iter().filter(|d| d.selected).count();
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        if let Some(ref action) = self.confirm_action.clone() {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    if action.starts_with("Move") {
                        self.move_dirs_to_trash();
                        self.start_scan();
                    } else if action == "Stop the current scan" {
                        self.scan_stop_signal.store(true, Ordering::SeqCst);
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
                KeyCode::Char('q') => std::process::exit(0),
                KeyCode::Esc => {
                    self.confirm_action = Some("Stop the current scan".to_string());
                }
                _ => {}
            },
            AppState::ScanComplete => match key.code {
                KeyCode::Char('q') | KeyCode::Esc => std::process::exit(0),
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

fn calculate_directory_size(path: &PathBuf) -> u64 {
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize terminal
    enable_raw_mode()?;
    let mut stdout = std::io::Stdout::lock(&stdout());
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run it
    let mut app = App::new();

    // Get directory argument or use current directory
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        let path = PathBuf::from(&args[1]);
        if path.is_dir() {
            app.current_directory = path;
        }
    }

    // Start the initial scan
    app.start_scan();

    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        // Handle scan updates
        if let Some(receiver) = &app.scan_receiver {
            if let Ok(update) = receiver.try_recv() {
                match update {
                    ScanUpdate::Path(path) => {
                        app.current_scan_path = Some(path);
                    }
                    ScanUpdate::Result(dir_info) => {
                        app.dirs_to_clean.push(dir_info);
                        app.dirs_to_clean.sort_by_key(|d| d.modified_days_ago);

                        app.scan_results.total_folders = app.dirs_to_clean.len();
                        app.scan_results.found_folders =
                            app.dirs_to_clean.iter().filter(|d| d.selected).count();
                        app.scan_results.total_size_gb = app
                            .dirs_to_clean
                            .iter()
                            .map(|d| d.size_bytes as f64)
                            .sum::<f64>()
                            / (1024.0 * 1024.0 * 1024.0);

                        if !app.dirs_to_clean.is_empty() && app.dir_list_state.selected().is_none()
                        {
                            app.dir_list_state.select(Some(0));
                        }
                    }
                    ScanUpdate::Done => {
                        app.state = AppState::ScanComplete;
                        app.scan_receiver = None;
                        app.current_scan_path = None;
                    }
                }
            }
        }

        // Handle input events
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    break;
                }
                app.handle_key_event(key);
            }
        }

        // Update spinner
        if matches!(app.state, AppState::Scanning) {
            app.spinner_index = (app.spinner_index + 1) % SPINNER_CHARS.len();
        }
    }

    // Cleanup
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}

fn ui(f: &mut Frame<'_>, app: &mut App) {
    let area = f.area();

    // Main layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(4),
        ])
        .split(area);

    // Top bar with directory info and scan results
    let dir_info = match app.state {
        AppState::Scanning => format!("Scanning: {}", app.current_directory.display()),
        AppState::ScanComplete => format!("Scanned: {}", app.current_directory.display()),
    };
    let scan_results_text = match app.state {
        AppState::Scanning => {
            let spinner = SPINNER_CHARS[app.spinner_index];
            let path_str = app
                .current_scan_path
                .as_ref()
                .map(|p| p.to_string_lossy())
                .unwrap_or_default();
            format!("{} {}", spinner, path_str)
        }
        AppState::ScanComplete => format!(
            "Scan completed {} folders, found {} folders",
            app.scan_results.total_folders, app.scan_results.found_folders
        ),
    };
    let top_paragraph = Paragraph::new(scan_results_text)
        .block(Block::default().title(dir_info).borders(Borders::ALL));
    f.render_widget(top_paragraph, chunks[0]);

    // Content area
    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(chunks[1]);

    // Left panel area
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(content_chunks[0]);

    // Top-left panel - folders to clean
    let mut folder_items = Vec::new();
    for (i, folder) in app.folders_to_clean.iter().enumerate() {
        let checked = if app.selected_folders[i] {
            "[x]"
        } else {
            "[ ]"
        };
        folder_items.push(ListItem::new(format!("{} {}", checked, folder)));
    }

    let folders_list = List::new(folder_items)
        .block(
            Block::default()
                .title("Folders to clean")
                .borders(Borders::ALL),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    f.render_widget(folders_list, left_chunks[0]);

    // Bottom-left panel - ignore patterns
    let ignore_items: Vec<ListItem> = app
        .ignore_patterns
        .iter()
        .map(|p| ListItem::new(p.as_str()))
        .collect();

    let ignore_list = List::new(ignore_items).block(
        Block::default()
            .title("Ignore Patterns")
            .borders(Borders::ALL),
    );
    f.render_widget(ignore_list, left_chunks[1]);

    // Right panel - files to clean
    let mut file_items = Vec::new();

    if app.dirs_to_clean.is_empty() {
        if matches!(app.state, AppState::ScanComplete) {
            file_items.push(ListItem::new("No matching directories found"));
        }
        // else: show nothing while scanning
    } else {
        for dir in app.dirs_to_clean.iter() {
            let checked = if dir.selected { "[x]" } else { "[ ]" };

            // Format directory size for display
            let size_text = if dir.size_bytes < 1024 {
                format!("{} B", dir.size_bytes)
            } else if dir.size_bytes < 1024 * 1024 {
                format!("{} KB", dir.size_bytes / 1024)
            } else if dir.size_bytes < 1024 * 1024 * 1024 {
                format!("{} MB", dir.size_bytes / (1024 * 1024))
            } else {
                format!(
                    "{:.1} GB",
                    dir.size_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
                )
            };

            // Show size and full path instead of just folder name
            let item_text = format!("{} {} → {}", checked, size_text, dir.path.display());

            let item = ListItem::new(item_text);
            file_items.push(item);
        }
    }

    // Create list widget for directories
    let dirs_list = List::new(file_items)
        .block(
            Block::default()
                .title("Directories to clean")
                .borders(Borders::ALL),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    f.render_stateful_widget(dirs_list, content_chunks[1], &mut app.dir_list_state);

    // Bottom panel - instructions
    let help_text = "ESC: cancel/quit | ↑/↓: up/down | Space: toggle selection \na/d: select/deselect all | c: clean selected";
    let help_block = Block::default()
        .title("Instructions")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue));
    let help_paragraph = Paragraph::new(help_text).block(help_block);

    f.render_widget(help_paragraph, chunks[2]);

    // Handle confirmation
    if let Some(ref action) = app.confirm_action {
        let confirm_text = format!("{}? (Y/n)", action);
        let confirm_block = Block::default()
            .title("Confirm Action")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Red));
        let confirm_paragraph = Paragraph::new(confirm_text)
            .block(confirm_block)
            .style(Style::default().bg(Color::DarkGray));

        // Calculate position to center the confirmation message
        let text_width = action.len() as u16 + 8; // approx width for action + "? (Y/n)"
        let area_width = area.width;
        let area_height = area.height;
        let popup_width = std::cmp::min(text_width + 4, area_width.saturating_sub(4));
        let popup_height = 5; // Increased height for better formatting

        let confirm_area = Rect {
            x: area.x + (area_width.saturating_sub(popup_width)) / 2,
            y: area.y + (area_height.saturating_sub(popup_height)) / 2,
            width: popup_width,
            height: popup_height,
        };

        f.render_widget(ratatui::widgets::Clear, confirm_area); // Clear the area before drawing
        f.render_widget(confirm_paragraph, confirm_area);
    }
}
