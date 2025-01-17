use clap::{Arg, Command};
use git2::{DiffOptions, Repository};
use reqwest::Client;
use serde_json::json;
use std::process;
use anyhow::{Context, Result};  

#[tokio::main]
async fn main() {
    let matches = Command::new("git-commit-helper")
        .version("1.0")
        .about("Generate a commit message based on diffs in the working directory")
        .arg(
            Arg::new("directory")
                .help("Path to the Git repository directory")
                .required(true),
        )
        .get_matches();

    let directory = matches.get_one::<String>("directory").expect("directory is required");

    let repo = match Repository::discover(directory) {
        Ok(repo) => repo,
        Err(e) => {
            eprintln!("Error: Failed to open Git repository: {}", e);
            process::exit(1);
        }
    };

    let diffs = match fetch_diffs(&repo) {
        Ok(diffs) => diffs,
        Err(e) => {
            eprintln!("Error: {}", e);
            process::exit(1);
        }
    };

    match send_to_groq(diffs).await {
        Ok(response) => {
            println!("Suggested commit message:\n{}", response);
        }
        Err(e) => {
            eprintln!("Error: Failed to fetch response from GROQ API: {}", e);
            process::exit(1);
        }
    }
}

fn fetch_diffs(repo: &Repository) -> Result<String, git2::Error> {
    let mut diff_options = DiffOptions::new();
    diff_options.ignore_whitespace_change(true);

    let diff = repo.diff_index_to_workdir(None, Some(&mut diff_options))?;

    let mut diff_text = String::new();
    diff.print(git2::DiffFormat::Patch, |_, _, line| {
        diff_text.push_str(std::str::from_utf8(line.content()).unwrap_or("(unreadable diff)"));
        true
    })?;

    if diff_text.is_empty() {
        return Err(git2::Error::from_str(
            "No changes found in the working directory.",
        ));
    }

    Ok(diff_text)
}

async fn send_to_groq(diffs: String) -> Result<String> {
    let groq_api_url = "https://api.groq.com/openai/v1/chat/completions";
    let groq_api_key = ""; // hard coded for now will use an env variable later

    let payload = json!({
        "messages": [
            {
                "role": "system",
                "content": "You are an AI coding assistant helping generate a concise and accurate Git commit message based solely on code diffs. Focus on meaningful changes in code, such as the removal of dependencies, changes to logic or imports, and modifications to function signatures. Do not mention updates to dependency versions unless they are part of the changes, and avoid including formatting or whitespace changes."
            },
            {
                "role": "user",
                "content": format!("Here are the changes in the code:\n{}", diffs)
            }
        ],
        "model": "llama-3.3-70b-versatile",
        "temperature": 1.0,
        "max_completion_tokens": 256,
        "top_p": 1,
        "stream": false
    });       

    let client = Client::new();
    let response = client
        .post(groq_api_url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", groq_api_key))
        .json(&payload)
        .send()
        .await
        .context("Failed to send request to GROQ API")?;

    let json_response: serde_json::Value = response.json().await.context("Failed to parse response JSON")?;

    if let Some(commit_message) = json_response["choices"][0]["message"]["content"].as_str() {
        Ok(commit_message.trim().to_string())
    } else {
        Err(anyhow::anyhow!("Failed to extract commit message from the response").into())
    }
}