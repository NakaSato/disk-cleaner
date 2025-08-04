# Disk Cleaner

A terminal user interface (TUI) application built in Rust that efficiently finds and deletes project artifact folders (like `node_modules` or `target`) to help you reclaim disk space.

---

## Core Functionality

The application provides a single, dynamic interface that updates in real-time. It's designed to be fast, interactive, and intuitive.

### Dynamic UI Layout

The screen is divided into several panels that provide information and interactivity:

1.  **Status Bar (Top)**:
    *   Displays the current operation: `Scanning`, `Stopping`, `Scanned`, or `Deletion Complete`.
    *   During a scan, it shows an animated spinner and the path of the directory currently being examined.
    *   After a scan, it provides a summary of the total folders found.

2.  **Configuration Panel (Left)**:
    *   This panel is split vertically.
    *   **Folders to Clean**: A static list of folder names the application is configured to search for (e.g., `node_modules`, `target`).
    *   **Ignore Patterns**: A list of glob patterns for directories to ignore during the scan (e.g., `.*` to ignore hidden directories).

3.  **Results Panel (Right)**:
    *   Displays the list of found directories **in real-time** as the scan progresses.
    *   Each entry shows its selection status (`[x]` or `[ ]`), human-readable size, and full path.
    *   The list is automatically sorted with the **oldest folders appearing first**.
    *   The title dynamically updates to show the total size of all currently selected folders.

4.  **Instructions Bar (Bottom)**:
    *   Provides a quick reference for all available keyboard shortcuts.

### Asynchronous Scanning & Selection

*   **Asynchronous Scan**: The directory scan runs on a background thread, so the UI remains responsive at all times.
*   **Recursive Search**: The scan starts from the current directory or a path provided as a command-line argument (e.g., `disk-cleaner ./my-projects`).
*   **Automatic Selection**: Folders that were last modified **more than 30 days ago** are automatically selected for deletion by default.

---

## Controls

*   `↑`/`↓` **Arrow Keys**: Navigate the list of found directories.
*   **Spacebar**: Manually select or deselect the highlighted directory.
*   `a` / `d`: Select / Deselect all directories in the list.
*   `c` or `Enter`: Proceed to confirm the deletion of selected items.
*   `Esc`:
    *   During a scan, it opens a confirmation dialog to stop the process.
    *   At any other time, it will quit the application.
*   `q`: Quit the application at any time.

---

## Dialogs

The application uses contextual pop-up dialogs for important actions:

1.  **Stop Scan Confirmation**:
    *   Triggered by `Esc` during a scan.
    *   Asks: `Stop the current scan? (Y/n)`
    *   `Y`: Stops the scan immediately and displays all results found up to that point.
    *   `N`: Closes the dialog and resumes the scan.

2.  **Deletion Confirmation**:
    *   Triggered by `c` or `Enter` when items are selected.
    *   Asks: `Move X selected items to trash? (Y/n)`
    *   `Y`: Moves the selected folders to the system's trash bin.
    *   `N`: Cancels the operation and returns to the list view.

3.  **Deletion Summary**:
    *   Appears after a successful deletion.
    *   Summarizes the number of folders cleaned and the total space freed.
    *   Prompts the user to press `y` or `enter` to exit the application.

---

## Technical & Implementation Details

*   **Language**: **Rust**
*   **Crates**:
    *   **`ratatui`** with the **`crossterm`** backend for the TUI.
    *   **`trash`** for safely moving items to the system trash.
    *   **`walkdir`** for recursive directory traversal.
    *   **`glob`** for matching ignore patterns.
*   **Architecture**:
    *   The application is built with a modular structure, separating logic into `main.rs` (entry point), `app.rs` (state management), `ui.rs` (rendering), and `scanner.rs` (file system logic).
*   **Error Handling**:
    *   The application is designed to handle errors gracefully (e.g., permission issues) without crashing.

---

## Testing Requirements

*   Unit tests should cover the business logic, not UI rendering.
*   **Test Scope**:
    *   Logic for parsing command-line arguments.
    *   Recursive directory scanning function.
    *   Filtering logic that correctly identifies folders older than 30 days.
*   **Methodology**: Tests should create temporary directories and files to simulate a real file system, ensuring tests are hermetic and don't affect the user's actual files.

---

## Future Improvements

Here are some potential features and enhancements for the future:

*   **Configuration File (`cleaner.toml`)**:
    *   Load "folders to clean" and "ignore patterns" from a user-defined configuration file instead of having them hardcoded.
*   **Interactive Configuration**:
    *   Allow users to toggle which folders to scan and to add/remove ignore patterns directly within the UI.
*   **Error and Logging Panel**:
    *   Add a dedicated panel or log file to display errors, such as when a folder cannot be deleted due to permission issues.
*   **More Granular Time-Based Filtering**:
    *   Allow users to specify a custom time frame for automatic selection (e.g., "older than 7 days," "older than 6 months") via a command-line argument or UI setting.
*   **Performance Optimizations for Size Calculation**:
    *   Use parallel processing or lazy calculation to speed up the directory size calculation, which can be slow for large folders like `node_modules`.

---

## Related

- https://github.com/Byron/dua-cli
