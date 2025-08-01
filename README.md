# Cleaner

## Instructions

### **Project Goal**

Create a terminal user interface (TUI) application in Rust named `cleaner-rs` that finds and deletes specified project artifact folders (like `node_modules` or `target`) based on their last modified date.

---

### **Core Functionality**

The application should operate in three main states:

1.  **State 1: Configuration Screen (Initial View)**
    * Display a list with selectable checkboxes for folder names to search for.
    * **Options**: `node_modules` and `target`.
    * **Default**: Both options should be **checked** (`[x]`) by default.
    * **Controls**:
        * `Up/Down Arrow Keys`: Navigate between options.
        * `Spacebar`: Toggle the selected option (`[x]` or `[ ]`).
        * `Enter`: Confirm selection and proceed to the next state.

2.  **State 2: Scanning & Selection Screen**
    * **Scanning Logic**:
        * If the app is run with a path argument (e.g., `cleaner-rs ./projects`), it should scan that directory recursively.
        * If run with no arguments, it should scan the current working directory recursively.
        * The scan should only search for folder names selected in State 1.
    * **Display**:
        * Show a list of all found folders.
        * For each folder, display its full path, and its last modified time in a human-readable format (e.g., "3 days ago", "5 months ago").
        * Sort the list with the **oldest folders appearing first**.
    * **Selection Logic**:
        * By default, automatically select all folders that were last modified **more than 30 days ago**.
    * **Controls**:
        * `Up/Down Arrow Keys`: Navigate the list of folders.
        * `Spacebar`: Manually select/unselect a folder.
        * `Enter`: Proceed to the confirmation state.
        * `c`: Delete selected folders (move to trash).
        * `c`: Delete selected folders (move to trash).

3.  **State 3: Confirmation & Deletion**
    * **Confirmation Prompt**: After the user hits `Enter` in State 2, display a clear confirmation message.
        * Example: `Move 5 selected items to trash? (Y/n)`
    * **Action**:
        * If the user presses `Y` (or `y`), move each selected folder to the **system's trash/recycle bin**. Do **not** permanently delete.
        * If the user presses `N` (or `n`), abort the operation and return to the selection screen (State 2).
        * The application now shows scan results in the top bar: "Scanned: . Scan completed X folders, found Y folders"
        * Shows folder size and full path instead of just folder name
        * Directory sizes displayed in human-readable format (B, KB, MB, GB)
        * Instructions are displayed properly in the bottom bar

        * ESC or 'n': Cancel confirmation dialog
        * Enter: Confirm deletion (not toggle selection)
---

### **Technical & Implementation Requirements**

* **Language & Crates**:
    * Use **Rust**.
    * Use **`ratatui`** for the TUI, with the **`crossterm`** backend.
    * Use the **`trash`** crate to safely move items to the system trash.
    * Use a crate like **`clap`** for parsing command-line arguments.
    * Use a crate like **`chrono`** or `std::time` for date/time calculations.
* **Global Controls**:
    * The user must be able to exit the application at any time by pressing `q` or `Esc`.
* **Error Handling**:
    * The application must not panic. Gracefully handle errors like file permission issues or invalid paths and display a message to the user in the TUI.

---

### **Testing Requirements**

* Write unit tests for the **business logic**, not the UI rendering.
* **Test Scope**:
    * The logic for parsing command-line arguments.
    * The recursive directory scanning function.
    * The filtering logic that correctly identifies folders older than 30 days.
* **Methodology**: Tests should create temporary directories and files to simulate a real file system, ensuring tests are hermetic and don't affect the user's actual files.

---
