mod cli;
mod config;
mod history;
mod menu;
mod ssh;

use clap::Parser;
use cli::{
    cmd_add_host, cmd_alias, cmd_tag, print_complete_suggestions, print_completions, Cli, Commands,
};
use config::{config_path, load_config, resolve_target, Config};
use menu::{build_menu, list_hosts, run_menu};
use ssh::interactive_session;
use std::env;
use std::error::Error;
use std::fs;
use std::path::Path;

/// Handle a `@group` target: validate the group, then run its scoped menu.
fn run_group_menu(
    zzh_folder: &Path,
    config: &Config,
    group: &str,
    detached: bool,
) -> Result<(), Box<dyn Error>> {
    if !config.groups.contains_key(group) {
        let names: Vec<&str> = config.groups.keys().map(String::as_str).collect();
        if names.is_empty() {
            eprintln!(
                "zzh: no groups defined in {}",
                config_path(zzh_folder).display()
            );
        } else {
            eprintln!(
                "zzh: unknown group '{group}'. Available: {}",
                names.join(", ")
            );
        }
        std::process::exit(1);
    }
    let entries = build_menu(zzh_folder, config, Some(group));
    run_menu(entries, config, detached, zzh_folder)
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    // Completions are a static script, emit them before creating ~/.zzh or
    // reading the config, so the command has no side effects.
    if let Some(Commands::Completions { shell }) = &cli.command {
        print_completions(shell);
        return Ok(());
    }

    let home_dir = env::var("HOME").expect("Could not determine the home directory");
    let zzh_folder = Path::new(&home_dir).join(".zzh");
    if !zzh_folder.exists() {
        fs::create_dir(&zzh_folder)?;
    }

    let config = load_config(&zzh_folder);

    if let Some(command) = cli.command {
        match command {
            Commands::Completions { .. } => unreachable!("handled before config load"),
            Commands::Complete { partial } => {
                print_complete_suggestions(partial, &zzh_folder, &config)
            }
            Commands::Alias { name, target } => cmd_alias(&zzh_folder, &name, &target)?,
            Commands::AddHost { name, host } => cmd_add_host(&zzh_folder, &name, host)?,
            Commands::Tag { group, hosts } => cmd_tag(&zzh_folder, &group, &hosts)?,
        }
        return Ok(());
    }

    ctrlc::set_handler(move || {}).expect("Error setting Ctrl-C handler");

    if cli.hosts {
        list_hosts(&config)?;
        return Ok(());
    }

    match cli.target {
        Some(target) => match target.strip_prefix('@') {
            Some(group) => run_group_menu(&zzh_folder, &config, group, cli.detached)?,
            None => {
                let resolved = resolve_target(&config, &target);
                interactive_session(&resolved, cli.detached, config.auto_reconnect, &zzh_folder)?;
            }
        },
        None => {
            let entries = build_menu(&zzh_folder, &config, None);
            run_menu(entries, &config, cli.detached, &zzh_folder)?;
        }
    }

    Ok(())
}
