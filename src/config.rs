use serde::Deserialize;
use std::collections::{BTreeMap, HashSet};
use std::env::var;
use std::fs;
use std::path::{Path, PathBuf};

const CONFIG_FILE: &str = "config.toml";

/// Commented example written to `~/.zzh/config.toml` on first run.
const CONFIG_TEMPLATE: &str = r#"# zzh configuration
#
# Define extra hosts, short aliases, and groups here. This file is read on
# every run. Edit it by hand or use `zzh add-host`, `zzh alias`, and `zzh tag`.

# Offer to reconnect when a session drops. On by default; set to false to exit
# as soon as the connection ends. Must stay above the tables below.
# auto_reconnect = true

# A host that isn't in ~/.ssh/config. Connect with `zzh db`.
# [hosts.db]
# hostname = "db.internal"                        # address ssh connects to
# user = "admin"                                  # optional -> admin@db.internal
# port = 2222                                     # optional -> -p 2222
# identity_file = "~/.ssh/id_db"                  # optional -> -i (~ expanded)
# proxy_jump = "bastion"                          # optional -> -J bastion
# options = ["StrictHostKeyChecking=accept-new"]  # optional -> repeated -o

# Short names. `zzh w` connects to web1.
# [aliases]
# w = "web1"

# Named host groups. `zzh @prod` scopes the menu to these.
# [groups]
# prod = ["web1", "web2", "db"]
"#;

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    pub hosts: BTreeMap<String, HostEntry>,
    pub aliases: BTreeMap<String, String>,
    pub groups: BTreeMap<String, Vec<String>>,
    pub auto_reconnect: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            hosts: BTreeMap::new(),
            aliases: BTreeMap::new(),
            groups: BTreeMap::new(),
            auto_reconnect: true,
        }
    }
}

/// A host definition; all fields optional. An entry with no `hostname` uses its
/// own name as the destination, letting ssh resolve it via `~/.ssh/config`.
#[derive(Debug, Default, Deserialize)]
pub struct HostEntry {
    hostname: Option<String>,
    user: Option<String>,
    port: Option<u16>,
    identity_file: Option<String>,
    proxy_jump: Option<String>,
    #[serde(default)]
    options: Vec<String>,
}

pub fn config_path(zzh_folder: &Path) -> PathBuf {
    zzh_folder.join(CONFIG_FILE)
}

/// Load the config, writing a template on first run. Errors are reported on
/// stderr but never abort the program.
pub fn load_config(zzh_folder: &Path) -> Config {
    let path = config_path(zzh_folder);

    if !path.exists() {
        if let Err(e) = fs::write(&path, CONFIG_TEMPLATE) {
            eprintln!("zzh: couldn't create {}: {e}", path.display());
        }
        return Config::default();
    }

    let contents = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("zzh: couldn't read {}: {e}", path.display());
            return Config::default();
        }
    };

    match toml::from_str(&contents) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("zzh: ignoring invalid {}: {e}", path.display());
            Config::default()
        }
    }
}

fn expand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Ok(home) = var("HOME") {
            return format!("{home}/{rest}");
        }
    }
    path.to_string()
}

/// A target resolved against the config: the `label` recorded in history and
/// the ssh `args` (flags followed by the destination).
pub struct Resolved {
    pub label: String,
    pub args: Vec<String>,
}

/// Resolve a token into ssh arguments: follow alias chains (guarding against
/// cycles), then expand a config host into flags. Unknown tokens pass through
/// to ssh unchanged.
pub fn resolve_target(config: &Config, target: &str) -> Resolved {
    let mut name = target.to_string();
    let mut seen = HashSet::new();
    while seen.insert(name.clone()) {
        match config.aliases.get(&name) {
            Some(next) => name = next.clone(),
            None => break,
        }
    }

    let args = match config.hosts.get(&name) {
        Some(host) => {
            let mut args = Vec::new();
            if let Some(port) = host.port {
                args.push("-p".to_string());
                args.push(port.to_string());
            }
            if let Some(identity) = &host.identity_file {
                args.push("-i".to_string());
                args.push(expand_tilde(identity));
            }
            if let Some(jump) = &host.proxy_jump {
                args.push("-J".to_string());
                args.push(jump.clone());
            }
            for opt in &host.options {
                args.push("-o".to_string());
                args.push(opt.clone());
            }
            let host_part = host.hostname.clone().unwrap_or_else(|| name.clone());
            let destination = match &host.user {
                Some(user) => format!("{user}@{host_part}"),
                None => host_part,
            };
            args.push(destination);
            args
        }
        None => vec![name],
    };

    Resolved {
        label: target.to_string(),
        args,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn cfg(toml_str: &str) -> Config {
        toml::from_str(toml_str).unwrap()
    }

    #[test]
    fn passthrough_unknown_target() {
        let resolved = resolve_target(&Config::default(), "example.com");
        assert_eq!(resolved.label, "example.com");
        assert_eq!(resolved.args, vec!["example.com".to_string()]);
    }

    #[test]
    fn alias_resolves_to_target() {
        let config = cfg("[aliases]\ndb = \"db.internal\"\n");
        let resolved = resolve_target(&config, "db");
        assert_eq!(resolved.args, vec!["db.internal".to_string()]);
        // History records what the user typed, not the expansion.
        assert_eq!(resolved.label, "db");
    }

    #[test]
    fn alias_cycle_terminates() {
        let config = cfg("[aliases]\na = \"b\"\nb = \"a\"\n");
        assert_eq!(resolve_target(&config, "a").args.len(), 1);
    }

    #[test]
    fn host_entry_builds_ssh_args() {
        let config = cfg(concat!(
            "[hosts.db]\n",
            "hostname = \"db.internal\"\n",
            "user = \"admin\"\n",
            "port = 2222\n",
            "proxy_jump = \"bastion\"\n",
            "options = [\"StrictHostKeyChecking=accept-new\"]\n",
        ));
        let resolved = resolve_target(&config, "db");
        let expected: Vec<String> = [
            "-p",
            "2222",
            "-J",
            "bastion",
            "-o",
            "StrictHostKeyChecking=accept-new",
            "admin@db.internal",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        assert_eq!(resolved.args, expected);
    }

    #[test]
    fn alias_into_host_entry() {
        let config = cfg("[hosts.db]\nhostname = \"db.internal\"\n[aliases]\nd = \"db\"\n");
        assert_eq!(
            resolve_target(&config, "d").args,
            vec!["db.internal".to_string()]
        );
    }

    #[test]
    fn host_without_hostname_uses_name() {
        let config = cfg("[hosts.web]\nuser = \"deploy\"\n");
        assert_eq!(
            resolve_target(&config, "web").args,
            vec!["deploy@web".to_string()]
        );
    }

    #[test]
    fn expand_tilde_uses_home() {
        env::set_var("HOME", "/home/u");
        assert_eq!(expand_tilde("~/.ssh/id"), "/home/u/.ssh/id");
        assert_eq!(expand_tilde("/abs/path"), "/abs/path");
        assert_eq!(expand_tilde("relative"), "relative");
    }

    #[test]
    fn auto_reconnect_defaults_on() {
        assert!(Config::default().auto_reconnect);
        assert!(cfg("[aliases]\nw = \"web1\"\n").auto_reconnect);
    }

    #[test]
    fn auto_reconnect_can_be_disabled() {
        assert!(!cfg("auto_reconnect = false\n").auto_reconnect);
    }

    #[test]
    fn groups_parse() {
        let config = cfg("[groups]\nprod = [\"web1\", \"db\"]\n");
        assert_eq!(
            config.groups["prod"],
            vec!["web1".to_string(), "db".to_string()]
        );
    }
}
