use crate::app::{App, AppState};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

const SPINNER_CHARS: [char; 8] = ['⠁', '⠂', '⠄', '⡀', '⢀', '⠠', '⠐', '⠈'];

pub fn draw(f: &mut Frame<'_>, app: &mut App) {
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
        AppState::Stopping => format!("Stopping: {}", app.current_directory.display()),
        AppState::ScanComplete | AppState::DeletionComplete => {
            format!("Scanned: {}", app.current_directory.display())
        }
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
        AppState::Stopping => "Please wait...".to_string(),
        AppState::ScanComplete | AppState::DeletionComplete => format!(
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
    let title = if app.scan_results.selected_size_gb > 0.0 {
        format!(
            "Directories to clean: {:.2} GB selected",
            app.scan_results.selected_size_gb
        )
    } else {
        "Directories to clean".to_string()
    };
    let dirs_list = List::new(file_items)
        .block(Block::default().title(title).borders(Borders::ALL))
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

        f.render_widget(Clear, confirm_area); // Clear the area before drawing
        f.render_widget(confirm_paragraph, confirm_area);
    }

    // Handle Deletion Summary
    if let AppState::DeletionComplete = app.state {
        if let Some((count, size)) = app.deletion_summary {
            let size_gb = size as f64 / (1024.0 * 1024.0 * 1024.0);
            let summary_text = format!(
                "Cleaned {} folders, freeing {:.2} GB.\n\nPress 'y' or 'enter' to exit.",
                count, size_gb
            );
            let summary_block = Block::default()
                .title("Deletion Complete")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green));
            let summary_paragraph = Paragraph::new(summary_text)
                .block(summary_block)
                .style(Style::default().bg(Color::DarkGray))
                .alignment(ratatui::layout::Alignment::Center);

            let area_width = area.width;
            let area_height = area.height;
            let popup_width = 50;
            let popup_height = 7;

            let summary_area = Rect {
                x: area.x + (area_width.saturating_sub(popup_width)) / 2,
                y: area.y + (area_height.saturating_sub(popup_height)) / 2,
                width: popup_width,
                height: popup_height,
            };

            f.render_widget(Clear, summary_area);
            f.render_widget(summary_paragraph, summary_area);
        }
    }
}
