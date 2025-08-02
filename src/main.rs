mod app;
mod scanner;
mod ui;

use crate::app::{App, AppState, ScanUpdate};
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{io, path::PathBuf, time::Duration};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
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
        terminal.draw(|f| ui::draw(f, &mut app))?;

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
                        app.update_selection_scan_results();
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
        if app.state == AppState::Scanning {
            // A bit of a hack to access the spinner length
            const SPINNER_LEN: usize = 8;
            app.spinner_index = (app.spinner_index + 1) % SPINNER_LEN;
        }
    }

    // Cleanup
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
