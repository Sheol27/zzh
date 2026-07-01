use crate::config::Resolved;
use crate::history::append_to_history;
use dialoguer::Confirm;
use regex::Regex;
use std::env::var;
use std::error::Error;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::{thread, time};

fn find_config() -> Result<PathBuf, std::env::VarError> {
    let home = var("HOME")?;
    Ok(Path::new(&home).join(".ssh").join("config"))
}

/// List the hosts defined in `~/.ssh/config`, skipping wildcard patterns.
pub fn extract_hosts() -> Result<Vec<String>, Box<dyn Error>> {
    let config_path = find_config()?;
    let mut file = File::open(&config_path)?;

    let mut s = String::new();
    file.read_to_string(&mut s)?;

    let re = Regex::new(r"(?im)^[ \t]*Host[ \t]+(.+)$").unwrap();
    let hosts: Vec<String> = re
        .captures_iter(&s)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str()))
        .flat_map(|line| {
            line.split('#')
                .next()
                .unwrap_or("")
                .split_whitespace()
                .filter(|h| !h.contains('*') && !h.contains('?'))
                .map(|h| h.to_string())
                .collect::<Vec<_>>()
        })
        .collect();

    Ok(hosts)
}

/// Open an ssh session over a persistent control socket, offering to reconnect
/// when the connection drops.
pub fn interactive_session(
    resolved: &Resolved,
    detached: bool,
    auto_reconnect: bool,
    folder: &Path,
) -> Result<(), Box<dyn Error>> {
    let mut command = std::process::Command::new("ssh");
    let str_folder = folder.to_str().unwrap();

    command
        .args(["-o", "ControlMaster=auto"])
        .args(["-o", &format!("ControlPath={str_folder}/%r@%h:%p")])
        .args(["-o", "ControlPersist=yes"]);

    if detached {
        command.arg("-fN").arg("-T");
    }
    command.args(&resolved.args);
    append_to_history(folder, &resolved.label)?;

    let target = &resolved.label;
    const MAX_AUTO_ATTEMPTS: u32 = 5;
    let mut total_attempts: u32 = 0;
    let mut auto_attempts_remaining: u32 = 0;
    loop {
        let status = command.status()?;
        if status.success() || !auto_reconnect {
            break;
        }

        let exit_code = status.code().unwrap_or(-1);
        if auto_attempts_remaining == 0 {
            let prompt = if total_attempts == 0 {
                format!("Connection to {target} ended (exit {exit_code}). Reconnect?")
            } else {
                format!("Gave up after {total_attempts} attempts (exit {exit_code}). Try again?")
            };

            let should_reconnect = Confirm::new()
                .with_prompt(prompt)
                .default(false)
                .interact()
                .unwrap_or(false);

            if !should_reconnect {
                break;
            }
            auto_attempts_remaining = MAX_AUTO_ATTEMPTS;
        }

        total_attempts += 1;
        auto_attempts_remaining -= 1;
        println!("Reconnecting... attempt {total_attempts}");
        thread::sleep(time::Duration::from_millis(1000));
    }

    Ok(())
}
