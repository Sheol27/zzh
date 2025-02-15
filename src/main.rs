use clap::Parser;
use dialoguer::theme::ColorfulTheme;
use dialoguer::FuzzySelect;
use regex::Regex;
use std::env::{self, var};
use std::error::Error;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

/// Returns the path to the SSH config file (currently `$HOME/.ssh/config`)
fn find_config() -> Result<PathBuf, std::env::VarError> {
    let home = var("HOME")?;
    Ok(Path::new(&home).join(".ssh").join("config"))
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
fn interactive_session(target: &str, detached: bool, folder: &str) -> Result<(), Box<dyn Error>> {
    let mut command = std::process::Command::new("ssh");

    command
        .args(&["-o", "ControlMaster=auto"])
        .args(&["-o", &format!("ControlPath={folder}/%r@%h:%p")])
        .args(&["-o", "ControlPersist=yes"]);

    if detached {
        command.arg("-fN").arg("-T");
    }
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

    ///  Instanciate the socket without interactive shell
    #[arg(long)]
    detached: bool,

    /// List hosts from ssh config
    #[arg(long)]
    hosts: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    let home_dir = env::var("HOME").expect("Could not determine the home directory");
    let zzh_folder = Path::new(&home_dir).join(".zzh");
    let zzh_folder_string = zzh_folder.to_str().unwrap();

    if !zzh_folder.exists() {
        fs::create_dir(&zzh_folder)?;
    }

    if cli.hosts {
        list_hosts()?;
        return Ok(());
    }

    if let Some(target) = cli.target {
        interactive_session(&target, cli.detached, zzh_folder_string)?;
    } else {
        let hosts = extract_hosts()?;

        let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
            .default(0)
            .highlight_matches(true)
            .items(&hosts)
            .interact()
            .unwrap();
        interactive_session(&hosts[selection], cli.detached, zzh_folder_string)?;
    }

    Ok(())
}
