use crate::config::{config_path, Config};
use crate::menu::build_menu;
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::error::Error;
use std::fs;
use std::path::Path;
use toml_edit::{value, Array, DocumentMut, Item, Table, Value};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// The target host to connect to (use @group to scope the menu to a group)
    pub target: Option<String>,

    /// Instantiate the socket without interactive shell
    #[arg(long)]
    pub detached: bool,

    /// List hosts from ssh config and the zzh config
    #[arg(long)]
    pub hosts: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Clone, ValueEnum)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
}

/// ssh-connection options for `add-host`, flattened into the subcommand.
#[derive(Args)]
pub struct HostArgs {
    /// Address ssh connects to (defaults to the name)
    #[arg(long)]
    hostname: Option<String>,
    /// Remote user
    #[arg(long)]
    user: Option<String>,
    /// Remote port
    #[arg(long)]
    port: Option<u16>,
    /// Identity file (-i)
    #[arg(long, value_name = "FILE")]
    identity: Option<String>,
    /// Jump host (-J)
    #[arg(long, value_name = "HOST")]
    jump: Option<String>,
    /// Extra ssh option (-o); repeatable
    #[arg(long = "option", value_name = "OPT")]
    options: Vec<String>,
}

#[derive(Subcommand)]
pub enum Commands {
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
    /// Define a short alias for a target
    Alias {
        /// Alias name (what you'll type)
        name: String,
        /// Target it resolves to (a host name, ssh-config host, or user@host)
        target: String,
    },
    /// Add or update a host definition in the zzh config
    AddHost {
        /// Name you'll connect with
        name: String,
        #[command(flatten)]
        host: HostArgs,
    },
    /// Add one or more hosts to a group (creating it if needed)
    Tag {
        /// Group name
        group: String,
        /// Hosts to add to the group
        #[arg(required = true)]
        hosts: Vec<String>,
    },
}

pub fn print_completions(shell: &Shell) {
    let script = match shell {
        Shell::Bash => {
            r#"_zzh() {
    local cur="${COMP_WORDS[COMP_CWORD]}"
    if [[ ${COMP_CWORD} -eq 1 ]]; then
        COMPREPLY=($(compgen -W "$(zzh _complete "$cur" 2>/dev/null)" -- "$cur"))
    elif [[ "${COMP_WORDS[1]}" == "completions" && ${COMP_CWORD} -eq 2 ]]; then
        COMPREPLY=($(compgen -W "bash zsh fish" -- "$cur"))
    fi
}
complete -F _zzh zzh"#
        }
        Shell::Zsh => {
            r#"#compdef zzh

_zzh() {
    local -a hosts
    if (( CURRENT == 2 )); then
        hosts=(${(f)"$(zzh _complete "${words[2]}" 2>/dev/null)"})
        _describe 'hosts' hosts
    elif [[ "${words[2]}" == "completions" ]] && (( CURRENT == 3 )); then
        _values 'shell' bash zsh fish
    fi
}

compdef _zzh zzh"#
        }
        Shell::Fish => {
            r#"complete -c zzh -f
complete -c zzh -n "__fish_is_first_token" -a "(zzh _complete (commandline -ct) 2>/dev/null)"
complete -c zzh -n "__fish_seen_subcommand_from completions" -a "bash zsh fish""#
        }
    };
    println!("{script}");
}

/// Output matching hosts (or `@group` names) for shell completion.
pub fn print_complete_suggestions(partial: Option<String>, zzh_folder: &Path, config: &Config) {
    let raw = partial.unwrap_or_default();

    if let Some(rest) = raw.strip_prefix('@') {
        let prefix = rest.to_lowercase();
        for name in config.groups.keys() {
            if name.to_lowercase().starts_with(&prefix) {
                println!("@{name}");
            }
        }
        return;
    }

    let prefix = raw.to_lowercase();
    for entry in build_menu(zzh_folder, config, None) {
        if entry.token.to_lowercase().starts_with(&prefix) {
            println!("{}", entry.token);
        }
    }
}

/// Read the config into an editable document (preserving comments), apply
/// `mutate`, and write it back.
fn edit_config<F>(zzh_folder: &Path, mutate: F) -> Result<(), Box<dyn Error>>
where
    F: FnOnce(&mut DocumentMut),
{
    let path = config_path(zzh_folder);
    let existing = fs::read_to_string(&path).unwrap_or_default();
    let mut doc = existing
        .parse::<DocumentMut>()
        .map_err(|e| format!("{} is not valid TOML: {e}", path.display()))?;
    mutate(&mut doc);
    fs::write(&path, doc.to_string())?;
    Ok(())
}

/// Set `table[key]` when `val` is present; leave the key untouched otherwise.
fn set_opt<T: Into<Value>>(table: &mut Table, key: &str, val: Option<T>) {
    if let Some(v) = val {
        table[key] = value(v);
    }
}

pub fn cmd_alias(zzh_folder: &Path, name: &str, target: &str) -> Result<(), Box<dyn Error>> {
    edit_config(zzh_folder, |doc| {
        let aliases = doc.entry("aliases").or_insert(Item::Table(Table::new()));
        if let Some(table) = aliases.as_table_mut() {
            table[name] = value(target);
        }
    })?;
    println!("aliased {name} -> {target}");
    Ok(())
}

pub fn cmd_add_host(zzh_folder: &Path, name: &str, host: HostArgs) -> Result<(), Box<dyn Error>> {
    edit_config(zzh_folder, |doc| {
        let hosts = doc.entry("hosts").or_insert(Item::Table(Table::new()));
        let Some(hosts_table) = hosts.as_table_mut() else {
            return;
        };
        // Emit sub-tables as `[hosts.<name>]` rather than an inline `[hosts]`.
        hosts_table.set_implicit(true);
        let entry = hosts_table.entry(name).or_insert(Item::Table(Table::new()));
        let Some(table) = entry.as_table_mut() else {
            return;
        };
        set_opt(table, "hostname", host.hostname);
        set_opt(table, "user", host.user);
        set_opt(table, "port", host.port.map(i64::from));
        set_opt(table, "identity_file", host.identity);
        set_opt(table, "proxy_jump", host.jump);
        if !host.options.is_empty() {
            table["options"] = value(host.options.into_iter().collect::<Array>());
        }
    })?;
    println!("added host {name}");
    Ok(())
}

pub fn cmd_tag(zzh_folder: &Path, group: &str, hosts: &[String]) -> Result<(), Box<dyn Error>> {
    edit_config(zzh_folder, |doc| {
        let groups = doc.entry("groups").or_insert(Item::Table(Table::new()));
        let Some(groups_table) = groups.as_table_mut() else {
            return;
        };
        let entry = groups_table.entry(group).or_insert(value(Array::new()));
        if let Some(array) = entry.as_array_mut() {
            for host in hosts {
                if !array.iter().any(|v| v.as_str() == Some(host.as_str())) {
                    array.push(host.as_str());
                }
            }
        }
    })?;
    println!("tagged {} into group {group}", hosts.join(", "));
    Ok(())
}
