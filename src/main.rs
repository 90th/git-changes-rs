use clap::{Arg, Command};
use git2::{DiffOptions, Repository};
use reqwest::Client;
use serde_json::json;
use anyhow::{Context, Result};
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    let matches = Command::new("git-commit-helper")
        .version("1.0")
        .about("Generate a commit message based on diffs in the working directory")
        .arg(
            Arg::new("directory")
                .help("Path to the Git repository directory")
                .required(true),
        )
        .get_matches();

    let directory = matches
        .get_one::<String>("directory")
        .context("Directory argument is required")?;

    let repo = Repository::discover(directory)
        .context("Failed to open Git repository")?;

    let diffs = fetch_diffs(&repo)
        .context("Failed to fetch diffs")?;

    let client = create_http_client();

    let response = send_to_groq(&client, diffs)
        .await
        .context("Failed to fetch response from GROQ API")?;

    println!("Suggested commit message:\n{}", response);

    Ok(())
}

fn create_http_client() -> Client {
    Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("Failed to create HTTP client")
}

fn fetch_diffs(repo: &Repository) -> Result<String> {
    let mut diff_options = DiffOptions::new();
    diff_options.ignore_whitespace_change(true);

    let diff = repo
        .diff_index_to_workdir(None, Some(&mut diff_options))
        .context("Failed to generate diff")?;

    let mut diff_text = String::new();
    diff.print(git2::DiffFormat::Patch, |_, _, line| {
        diff_text.push_str(std::str::from_utf8(line.content()).unwrap_or("(unreadable diff)"));
        true
    })
    .context("Failed to process diff output")?;

    if diff_text.is_empty() {
        anyhow::bail!("No changes found in the working directory");
    }

    Ok(diff_text)
}

async fn send_to_groq(client: &Client, diffs: String) -> Result<String> {
    let groq_api_url = "https://api.groq.com/openai/v1/chat/completions";

    let groq_api_key = env::var("GROQ_API_KEY")
        .context("Missing GROQ_API_KEY environment variable")?;

        let payload = json!({
            "messages": [
                {
                    "role": "system",
                    "content": "You are an AI coding assistant tasked with generating a concise and accurate Git commit message based solely on code diffs. The output should be structured with a title and a detailed body: \n\n\
        1. **Title**: A single-line summary that briefly describes the main purpose of the changes. Prefix the title with a conventional commit type (e.g., `fix`, `feat`, `refactor`, `docs`, etc.), followed by a concise description. \
        Use parentheses to indicate the area of the codebase affected (e.g., `fix(main)` or `refactor(cli)`).\n\n\
        2. **Body**: A detailed bullet-point list explaining each meaningful change. Use imperative verbs (e.g., 'remove', 'add', 'improve') to describe what was changed and why. \
        Include only meaningful changes (e.g., logic updates, dependency removals, refactoring) and omit irrelevant details (e.g., formatting or whitespace changes).\n\n\
        Do not speculate or include information that is not directly supported by the diffs. If dependencies are removed, note the removal. If imports are updated, specify them. Ensure the commit message is as accurate as possible, with no fabricated details."
                },
                {
                    "role": "user",
                    "content": format!("Here are the changes in the code:\n{}", diffs)
                }
            ],
            "model": "llama-3.3-70b-versatile",
            "temperature": 0.7,
            "max_completion_tokens": 256,
            "top_p": 1,
            "stream": false
        });
        

    let response = client
        .post(groq_api_url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", groq_api_key))
        .json(&payload)
        .send()
        .await
        .context("Failed to send request to GROQ API")?;

    let json_response: serde_json::Value = response
        .json()
        .await
        .context("Failed to parse response JSON")?;

    if let Some(commit_message) = json_response["choices"][0]["message"]["content"].as_str() {
        Ok(commit_message.trim().to_string())
    } else {
        Err(anyhow::anyhow!("Failed to extract commit message from the response"))
    }
}
