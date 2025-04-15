// src/main.rs
use anyhow::{anyhow, Context, Result};
use clap::{Arg, ArgAction, Command};
use dotenvy::dotenv;
use git2::{DiffDelta, DiffFormat, DiffOptions, Repository};
use glob::Pattern; // added for glob pattern matching
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use std::env;
use std::ffi::OsStr;
use std::path::Path;

#[derive(Deserialize, Debug)]
struct GeminiResponse {
    candidates: Option<Vec<Candidate>>,
}

#[derive(Deserialize, Debug)]
struct Candidate {
    content: Option<Content>,
    // unused fields removed
}

#[derive(Deserialize, Debug)]
struct Content {
    parts: Option<Vec<Part>>,
    // unused fields removed
}

#[derive(Deserialize, Debug)]
struct Part {
    text: Option<String>,
}

// safetyrating struct removed as it's no longer used by candidate

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    let matches = Command::new("git-changes-rs")
        .version("1.14")
        .about("Generate a commit message based on diffs using Gemini API")
        .arg(
            Arg::new("directory")
                .help("Path to the git repository directory")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::new("exclude")
                .short('e')
                .long("exclude")
                .help("Glob patterns to exclude (e.g., '*.log', 'target/**'). Use commas or multiple args.")
                .action(ArgAction::Append)
                .value_delimiter(',') // allow comma-separated values
                .value_name("PATTERNS"),
        )
        .get_matches();

    let directory = matches
        .get_one::<String>("directory")
        .context("directory argument is required")?;

    let mut excludes: Vec<String> = vec!["Cargo.lock".to_string()];
    if let Some(user_excludes) = matches.get_many::<String>("exclude") {
        excludes.extend(user_excludes.cloned());
    }
    println!(">>> main: excluding patterns: {:?}", excludes);

    let repo = Repository::discover(directory).context("failed to open git repository")?;

    println!("fetching diffs (filtering excluded files)...");
    let diffs = fetch_diffs(&repo, &excludes).context("failed to fetch diffs")?;

    if diffs.trim().is_empty() {
        println!(">>> main: no relevant changes found after fetch_diffs.");
        return Ok(());
    }

    println!(
        ">>> main: final filtered diffs found (len={})", // removed diff content print for brevity
        diffs.len()
    );

    let client = create_http_client();

    println!("generating commit message via gemini...");
    let response = send_to_gemini(&client, diffs)
        .await
        .context("failed to fetch response from gemini api")?;

    println!("\nsuggested commit message:\n---\n{}\n---", response);

    Ok(())
}

fn create_http_client() -> Client {
    Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .expect("failed to create http client")
}

// updated helper to use glob matching
fn is_excluded(delta: &DiffDelta, excludes: &[String]) -> bool {
    let check_path = |path_opt: Option<&Path>| -> bool {
        match path_opt {
            Some(p) => excludes.iter().any(|pattern_str| {
                match Pattern::new(pattern_str) {
                    Ok(pattern) => pattern.matches_path(p),
                    Err(e) => {
                        // warn if pattern is invalid but continue checking others
                        eprintln!("warning: invalid exclude pattern '{}': {}", pattern_str, e);
                        false
                    }
                }
            }),
            None => false, // if path doesn't exist, it can't match
        }
    };

    let old_path = delta.old_file().path();
    let new_path = delta.new_file().path();

    // if either the old path or new path matches an exclude pattern, exclude the delta
    check_path(old_path) || check_path(new_path)
}

fn fetch_diffs(repo: &Repository, excludes: &[String]) -> Result<String> {
    let mut diff_options = DiffOptions::new();
    diff_options.ignore_whitespace(true);

    let diff = repo
        .diff_index_to_workdir(None, Some(&mut diff_options))
        .context("failed to generate diff between index and workdir")?;

    let mut diff_text = String::new();
    let print_result = diff.print(DiffFormat::Patch, |delta, _hunk, line| {
        if !is_excluded(&delta, excludes) {
            match std::str::from_utf8(line.content()) {
                Ok(content) => diff_text.push_str(content),
                Err(_) => diff_text.push_str("(error: non-utf8 diff content)\n"),
            };
        }
        true
    });
    print_result.context("failed to process unstaged diff output with filtering")?;

    if diff_text.trim().is_empty() {
        let head_ref = repo.head().context("failed to get head reference")?;
        let head_tree = head_ref
            .peel_to_tree()
            .context("failed to peel head ref to tree")?;

        let staged_diff = repo
            .diff_tree_to_index(Some(&head_tree), None, Some(&mut diff_options))
            .context("failed to get diff between head tree and index")?;

        if staged_diff.deltas().len() > 0 {
            let mut staged_diff_text_local = String::new();
            let staged_print_result = staged_diff.print(DiffFormat::Patch, |delta, _hunk, line| {
                if !is_excluded(&delta, excludes) {
                    match std::str::from_utf8(line.content()) {
                        Ok(content) => staged_diff_text_local.push_str(content),
                        Err(_) => {
                            staged_diff_text_local.push_str("(error: non-utf8 diff content)\n")
                        }
                    };
                }
                true
            });
            staged_print_result.context("failed to process staged diff output with filtering")?;

            if !staged_diff_text_local.trim().is_empty() {
                diff_text = staged_diff_text_local;
            }
        }
    }

    if diff_text.trim().is_empty() {
        return Ok(String::new());
    }

    Ok(diff_text)
}

async fn send_to_gemini(client: &Client, diffs: String) -> Result<String> {
    let gemini_api_key =
        env::var("GEMINI_API_KEY").context("gemini_api_key not found in environment")?;
    let model_id = "gemini-2.0-flash";
    let api_url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
        model_id, gemini_api_key
    );

    let system_prompt = "You are an AI coding assistant that generates precise and structured Git commit messages. Your task is to produce **only** the commit title and body, following the **conventional commits** format (e.g., `fix(main)`, `feat(cli)`), using imperative verbs such as 'fix', 'add', 'remove'. The title should briefly summarize the change, followed by a detailed bullet-point list explaining the meaningful changes in the body. **Do not include any additional explanatory text** like the suggestion for what to include in the message or a recap of the format. Only return the commit message.";

    let payload = json!({
        "contents": [
            {
                "role": "user",
                "parts": [
                    {
                        "text": format!("Analyze the following Git diff carefully (excluding specified files like Cargo.lock, *.log, etc.) to understand the changes and generate a conventional commit message:\n\n```diff\n{}\n```", diffs)
                    }
                ]
            }
        ],
        "systemInstruction": {
            "parts": [
                { "text": system_prompt }
            ]
        },
        "generationConfig": {
            "temperature": 0.7,
            "topP": 1.0,
            "maxOutputTokens": 512,
            "responseMimeType": "text/plain"
        },
        "safetySettings": [
            {
                "category": "HARM_CATEGORY_CIVIC_INTEGRITY",
                "threshold": "BLOCK_NONE"
            }
        ]
    });

    let response = client
        .post(&api_url)
        .header("content-type", "application/json")
        .json(&payload)
        .send()
        .await
        .context("failed to send request to gemini api")?;

    let status = response.status();
    let response_body_text = response
        .text()
        .await
        .context("failed to read response body")?;

    if !status.is_success() {
        return Err(anyhow!(
            "gemini api returned status {}: {}",
            status,
            response_body_text
        ));
    }

    let gemini_response: GeminiResponse = serde_json::from_str(&response_body_text).context(
        format!("failed to parse json response: {}", response_body_text),
    )?;

    let commit_message = gemini_response
        .candidates
        .as_deref()
        .and_then(|c| c.first())
        .and_then(|c| c.content.as_ref())
        .and_then(|content| content.parts.as_deref())
        .and_then(|parts| parts.first())
        .and_then(|part| part.text.as_ref())
        .context("could not extract commit message text from gemini response")?;

    Ok(commit_message.trim().to_string())
}
