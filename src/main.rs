use chrono::prelude::*;
use clap::Parser;
use dialoguer::console::Style;
use dialoguer::console::Term;
use dialoguer::theme::ColorfulTheme;
use dialoguer::FuzzySelect;
use regex::Regex;
use std::collections::HashMap;
use std::env::{self, var};
use std::error::Error;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

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
        .map(|l| {
            let mut split = l.split(" ");

            let dt_str = split
                .next()
                .expect("Missing timestamp while processiing history");
            let target_str = split.next().expect("Missing host while processing history");

            let now_parsed: DateTime<Utc> = dt_str.parse().unwrap();

            (now_parsed, target_str)
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

    let re = Regex::new(r"Host\s+(\S+)").unwrap();

    let hosts: Vec<String> = re
        .captures_iter(&s)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
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

    append_to_history(&folder, target)?;
    let status = command.arg(target).status()?;
    if !status.success() {
        Err("ssh exited with an error")?
    }
    Ok(())
}

/// Reads the SSH config file and lists any defined hosts.
fn list_hosts() -> Result<(), Box<dyn Error>> {
    let config_file = find_config()?;
    let contents = fs::read_to_string(config_file)?;
    let re = Regex::new(r"^\s*Host\s+(.+)$")?;
    for line in contents.lines() {
        if let Some(caps) = re.captures(line) {
            let hosts = caps[1].trim();
            println!("{}", hosts);
        }
    }
    Ok(())
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
    ctrlc::set_handler(move || {}).expect("Error setting Ctrl-C handler");

    let home_dir = env::var("HOME").expect("Could not determine the home directory");
    let zzh_folder = Path::new(&home_dir).join(".zzh");

    if !zzh_folder.exists() {
        fs::create_dir(&zzh_folder)?;
    }

    if cli.hosts {
        list_hosts()?;
        return Ok(());
    }

    if let Some(target) = cli.target {
        interactive_session(&target, cli.detached, &zzh_folder)?;
    } else {
        let mut hosts = extract_hosts()?;
        let history = extract_history(&zzh_folder)?;

        hosts.retain(|x| !history.iter().any(|(key, _)| x == key));

        let mut options: Vec<(String, String)> = history
            .iter()
            .map(|(key, datetime)| {
                (
                    key.clone(),
                    format!("{} \x1b[90m({})\x1b[0m", key, datetime),
                )
            })
            .collect();

        for host in &hosts {
            options.push((host.clone(), host.clone()));
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
