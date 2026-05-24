use chrono::prelude::*;
use clap::{Parser, Subcommand, ValueEnum};
use dialoguer::console::Style;
use dialoguer::console::Term;
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, FuzzySelect};
use regex::Regex;
use std::collections::HashMap;
use std::env::{self, var};
use std::error::Error;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::{thread, time};

const HISTORY_FILE: &str = "history";

/// Returns the path to the SSH config file (currently `$HOME/.ssh/config`)
fn find_config() -> Result<PathBuf, std::env::VarError> {
    let home = var("HOME")?;
    Ok(Path::new(&home).join(".ssh").join("config"))
}

/// Extract connections history
fn extract_history(
    history_folder: &PathBuf,
) -> Result<Vec<(String, DateTime<Utc>)>, Box<dyn Error>> {
    let file_path = &history_folder.join(HISTORY_FILE);

    let mut s = String::new();
    match File::open(file_path) {
        Ok(mut f) => {
            f.read_to_string(&mut s)?;
        }
        Err(_) => {
            File::create(file_path)?;
        }
    };

    let data: Vec<(DateTime<Utc>, &str)> = s
        .lines()
        .filter_map(|l| {
            let mut split = l.split(' ');
            let dt_str = split.next()?;
            let target_str = split.next()?;
            if target_str.is_empty() {
                return None;
            }
            let parsed: DateTime<Utc> = dt_str.parse().ok()?;
            Some((parsed, target_str))
        })
        .collect();

    let mut latest_entries: HashMap<&str, DateTime<Utc>> = HashMap::new();

    for (datetime, key) in data {
        latest_entries
            .entry(key)
            .and_modify(|existing| {
                if *existing < datetime {
                    *existing = datetime;
                }
            })
            .or_insert(datetime);
    }

    let mut sorted_vec: Vec<(String, DateTime<Utc>)> = latest_entries
        .into_iter()
        .map(|(key, datetime)| (key.to_string(), datetime))
        .collect();

    sorted_vec.sort_by(|a, b| b.1.cmp(&a.1));

    Ok(sorted_vec)
}

/// Extract list of hosts from ssh config file
fn extract_hosts() -> Result<Vec<String>, Box<dyn Error>> {
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

/// Runs an interactive ssh session that opens a TTY on the remote host.
fn interactive_session(
    target: &str,
    detached: bool,
    folder: &PathBuf,
) -> Result<(), Box<dyn Error>> {
    let mut command = std::process::Command::new("ssh");
    let str_folder = folder.to_str().unwrap();

    command
        .args(&["-o", "ControlMaster=auto"])
        .args(&["-o", &format!("ControlPath={str_folder}/%r@%h:%p")])
        .args(&["-o", "ControlPersist=yes"]);

    if detached {
        command.arg("-fN").arg("-T");
    }
    command.arg(target);
    append_to_history(&folder, target)?;

    const MAX_AUTO_ATTEMPTS: u32 = 5;
    let mut total_attempts: u32 = 0;
    let mut auto_attempts_remaining: u32 = 0;
    loop {
        let status = command.status()?;

        if status.success() {
            break;
        }

        let exit_code = status.code().unwrap_or(-1);

        if auto_attempts_remaining == 0 {
            let prompt = if total_attempts == 0 {
                format!("Connection to {target} ended (exit {exit_code}). Reconnect?")
            } else {
                format!(
                    "Gave up after {total_attempts} attempts (exit {exit_code}). Try again?"
                )
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

/// Reads the SSH config file and lists any defined hosts.
fn list_hosts() -> Result<(), Box<dyn Error>> {
    for host in extract_hosts()? {
        println!("{}", host);
    }
    Ok(())
}

/// Get all hosts: history first (sorted by recency), then SSH config hosts not in history
fn get_all_hosts(zzh_folder: &Path) -> (Vec<(String, DateTime<Utc>)>, Vec<String>) {
    let history = extract_history(&zzh_folder.to_path_buf()).unwrap_or_default();
    let mut ssh_hosts = extract_hosts().unwrap_or_default();

    // Remove hosts that are already in history
    ssh_hosts.retain(|h| !history.iter().any(|(key, _)| key == h));

    (history, ssh_hosts)
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// The target host to connect to
    target: Option<String>,

    /// Instantiate the socket without interactive shell
    #[arg(long)]
    detached: bool,

    /// List hosts from ssh config
    #[arg(long)]
    hosts: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Clone, ValueEnum)]
enum Shell {
    Bash,
    Zsh,
    Fish,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate shell completions
    Completions {
        /// The shell to generate completions for
        shell: Shell,
    },
    /// Output matching hosts for shell completion (internal use)
    #[command(name = "_complete", hide = true)]
    Complete {
        /// Partial hostname to match
        partial: Option<String>,
    },
}

/// Print shell completions to stdout
fn print_completions(shell: Shell) {
    let script = match shell {
        Shell::Bash => r#"_zzh() {
    local cur="${COMP_WORDS[COMP_CWORD]}"
    if [[ ${COMP_CWORD} -eq 1 ]]; then
        COMPREPLY=($(compgen -W "$(zzh _complete "$cur" 2>/dev/null)" -- "$cur"))
    elif [[ "${COMP_WORDS[1]}" == "completions" && ${COMP_CWORD} -eq 2 ]]; then
        COMPREPLY=($(compgen -W "bash zsh fish" -- "$cur"))
    fi
}
complete -F _zzh zzh"#,
        Shell::Zsh => r#"#compdef zzh

_zzh() {
    local -a hosts
    if (( CURRENT == 2 )); then
        hosts=(${(f)"$(zzh _complete "${words[2]}" 2>/dev/null)"})
        _describe 'hosts' hosts
    elif [[ "${words[2]}" == "completions" ]] && (( CURRENT == 3 )); then
        _values 'shell' bash zsh fish
    fi
}

compdef _zzh zzh"#,
        Shell::Fish => r#"complete -c zzh -f
complete -c zzh -n "__fish_is_first_token" -a "(zzh _complete (commandline -ct) 2>/dev/null)"
complete -c zzh -n "__fish_seen_subcommand_from completions" -a "bash zsh fish""#,
    };
    println!("{}", script);
}

/// Output matching hosts for shell completion
fn print_complete_suggestions(partial: Option<String>, zzh_folder: &Path) {
    let prefix = partial.unwrap_or_default().to_lowercase();
    let (history, ssh_hosts) = get_all_hosts(zzh_folder);

    // Output matching hosts (history first, then SSH config)
    for (host, _) in history {
        if host.to_lowercase().starts_with(&prefix) {
            println!("{}", host);
        }
    }
    for host in ssh_hosts {
        if host.to_lowercase().starts_with(&prefix) {
            println!("{}", host);
        }
    }
}

/// Append connection to history with datetime
fn append_to_history(history_folder: &PathBuf, host: &str) -> Result<(), Box<dyn Error>> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&history_folder.join(HISTORY_FILE))
        .unwrap();

    let dt_now = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    if let Err(e) = writeln!(file, "{dt_now} {host}") {
        eprintln!("Couldn't write to file: {}", e);
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    if let Some(Commands::Completions { shell }) = cli.command {
        print_completions(shell);
        return Ok(());
    }

    let home_dir = env::var("HOME").expect("Could not determine the home directory");
    let zzh_folder = Path::new(&home_dir).join(".zzh");

    if !zzh_folder.exists() {
        fs::create_dir(&zzh_folder)?;
    }

    if let Some(Commands::Complete { partial }) = cli.command {
        print_complete_suggestions(partial, &zzh_folder);
        return Ok(());
    }

    ctrlc::set_handler(move || {}).expect("Error setting Ctrl-C handler");

    if cli.hosts {
        list_hosts()?;
        return Ok(());
    }

    if let Some(target) = cli.target {
        interactive_session(&target, cli.detached, &zzh_folder)?;
    } else {
        let (history, ssh_hosts) = get_all_hosts(&zzh_folder);

        let mut options: Vec<(String, String)> = history
            .iter()
            .map(|(key, datetime)| {
                (
                    key.clone(),
                    format!("{} \x1b[90m({})\x1b[0m", key, datetime),
                )
            })
            .collect();

        for host in ssh_hosts {
            options.push((host.clone(), host));
        }

        let display_options: Vec<&str> = options.iter().map(|opt| opt.1.as_str()).collect();
        let mut theme = ColorfulTheme::default();

        // FIXME: this is just a workaround, otherwise the datetime styling doesn't work.
        theme.active_item_style = Style::new();

        let term = Term::stdout();
        let selection = FuzzySelect::with_theme(&theme)
            .default(0)
            .highlight_matches(true)
            .items(&display_options)
            .interact_on(&term);

        match selection {
            Ok(value) => interactive_session(&options[value].0, cli.detached, &zzh_folder)?,
            Err(_) => {
                let _ = Term::stderr().show_cursor();
                ()
            }
        }
    }

    Ok(())
}
