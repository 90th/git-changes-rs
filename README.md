# git-changes-rs

A command-line tool written in Rust that analyzes Git diffs in your repository and uses the Google Gemini API to generate suggested commit messages following the Conventional Commits format.

This version uses the Gemini API, which generally provides more accurate and relevant commit message suggestions compared to previous iterations using other models.

## Features

* Generates conventional commit messages (title and body).
* Analyzes staged or unstaged changes.
* Uses the Google Gemini API for generation.
* Allows excluding files/patterns from the diff using glob patterns (e.g., `*.log`, `target/**`).
* Excludes `Cargo.lock` by default.

## Setup

1.  **Clone the repository:**
    ```bash
    git clone https://github.com/90th/git-changes-rs
    cd git-changes-rs
    ```
2.  **Build the project:**
    ```bash
    cargo build --release
    ```
    The executable will be located at `./target/release/git-changes-rs`.
3.  **Set API Key:**
    You need a Google Gemini API key. Set it as an environment variable:
    ```bash
    export GEMINI_API_KEY="YOUR_API_KEY_HERE"
    ```
    Alternatively, create a `.env` file in the project root (or the directory you run it from) with the line:
    ```
    GEMINI_API_KEY=YOUR_API_KEY_HERE
    ```

## Usage

Run the tool from your terminal, pointing it to the Git repository you want to analyze:

```bash
./target/release/git-changes-rs /path/to/your/repo [OPTIONS]
```

**Examples:**

* **Analyze current directory:**
    ```bash
    ./target/release/git-changes-rs .
    ```
* **Exclude log files and temp files:**
    ```bash
    # Using multiple arguments
    ./target/release/git-changes-rs . --exclude "*.log" --exclude "*.tmp"

    # Using comma-separated list
    ./target/release/git-changes-rs . --exclude "*.log,*.tmp"
    ```
* **Exclude a directory:**
    ```bash
    ./target/release/git-changes-rs . -e "dist/**"
    ```

The tool will print the suggested commit message to the console.

## Future Plans

* Looking into the creation of a VS Code / VisualStudio 2022 extension for easier integration (time permitting).
