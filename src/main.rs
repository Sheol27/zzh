use clap::Parser;
use regex::Regex;
use std::env::var;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

static FOLDER: &str = "/tmp/zh";

/// Returns the path to the SSH config file (currently `$HOME/.ssh/config`)
fn find_config() -> Result<PathBuf, std::env::VarError> {
    let home = var("HOME")?;
    Ok(Path::new(&home).join(".ssh").join("config"))
}

/// Runs an interactive ssh session that opens a TTY on the remote host.
fn interactive_session(target: &str, detached: bool) -> Result<(), Box<dyn Error>> {
    let mut command = std::process::Command::new("ssh");

    command
        .args(&["-o", "ControlMaster=auto"])
        .args(&["-o", &format!("ControlPath={FOLDER}/%r@%h:%p")])
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
    #[arg(required_unless_present_any = ["hosts"])]
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

    if !Path::new(FOLDER).exists() {
        fs::create_dir(FOLDER)?;
    }

    if cli.hosts {
        list_hosts()?;
        return Ok(());
    }

    if let Some(target) = cli.target {
        smol::block_on(async {
            interactive_session(&target, cli.detached)?;
            Ok::<(), Box<dyn Error>>(())
        })?;
    }

    Ok(())
}
