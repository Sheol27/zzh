use crate::config::{resolve_target, Config};
use crate::history::extract_history;
use crate::ssh::{extract_hosts, interactive_session};
use chrono::{DateTime, Utc};
use dialoguer::console::{Style, Term};
use dialoguer::theme::ColorfulTheme;
use dialoguer::FuzzySelect;
use std::collections::{BTreeMap, HashSet};
use std::error::Error;
use std::path::Path;

pub struct MenuEntry {
    pub token: String,
    pub display: String,
}

fn menu_display(token: &str, detail: &str) -> String {
    if detail.is_empty() {
        token.to_string()
    } else {
        format!("{token} \x1b[90m({detail})\x1b[0m")
    }
}

/// List every host zzh knows about: ssh-config hosts, then config hosts and
/// aliases, de-duplicated.
pub fn list_hosts(config: &Config) -> Result<(), Box<dyn Error>> {
    let mut seen: HashSet<String> = HashSet::new();
    let names = extract_hosts()
        .unwrap_or_default()
        .into_iter()
        .chain(config.hosts.keys().cloned())
        .chain(config.aliases.keys().cloned());
    for name in names {
        if seen.insert(name.clone()) {
            println!("{name}");
        }
    }
    Ok(())
}

pub fn build_menu(zzh_folder: &Path, config: &Config, group: Option<&str>) -> Vec<MenuEntry> {
    let history = extract_history(zzh_folder).unwrap_or_default();
    let ssh_hosts = extract_hosts().unwrap_or_default();
    assemble_menu(&history, &ssh_hosts, config, group)
}

/// Order: history (by recency) -> config aliases + hosts -> ssh-config hosts,
/// de-duplicated and annotated with group membership. With `group` set, only
/// that group's members are shown, including a member defined solely by the
/// grouping, so `@group` can always reach it.
fn assemble_menu(
    history: &[(String, DateTime<Utc>)],
    ssh_hosts: &[String],
    config: &Config,
    group: Option<&str>,
) -> Vec<MenuEntry> {
    let mut groups_of: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (name, members) in &config.groups {
        for member in members {
            groups_of.entry(member.as_str()).or_default().push(name);
        }
    }
    let group_label = |token: &str| -> String {
        groups_of
            .get(token)
            .map(|g| g.join(", "))
            .unwrap_or_default()
    };

    // An unknown group resolves to no members, hence an empty menu.
    let empty: &[String] = &[];
    let members: Option<&[String]> =
        group.map(|g| config.groups.get(g).map_or(empty, Vec::as_slice));
    let filter: Option<HashSet<&str>> = members.map(|m| m.iter().map(String::as_str).collect());
    let included = |token: &str| filter.as_ref().is_none_or(|f| f.contains(token));

    let mut entries: Vec<MenuEntry> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for (token, datetime) in history {
        if !included(token) || !seen.insert(token.clone()) {
            continue;
        }
        let groups = group_label(token);
        let detail = if groups.is_empty() {
            datetime.to_string()
        } else {
            format!("{datetime}, {groups}")
        };
        entries.push(MenuEntry {
            token: token.clone(),
            display: menu_display(token, &detail),
        });
    }

    for token in config.aliases.keys().chain(config.hosts.keys()) {
        if !included(token) || !seen.insert(token.clone()) {
            continue;
        }
        entries.push(MenuEntry {
            token: token.clone(),
            display: menu_display(token, &group_label(token)),
        });
    }

    for token in ssh_hosts {
        if !included(token) || !seen.insert(token.clone()) {
            continue;
        }
        entries.push(MenuEntry {
            token: token.clone(),
            display: menu_display(token, &group_label(token)),
        });
    }

    if let Some(members) = members {
        for token in members {
            if !seen.insert(token.clone()) {
                continue;
            }
            entries.push(MenuEntry {
                token: token.clone(),
                display: menu_display(token, &group_label(token)),
            });
        }
    }

    entries
}

/// Present the fuzzy-select menu and connect to the chosen entry.
pub fn run_menu(
    entries: Vec<MenuEntry>,
    config: &Config,
    detached: bool,
    zzh_folder: &Path,
) -> Result<(), Box<dyn Error>> {
    if entries.is_empty() {
        eprintln!("zzh: no hosts to show");
        return Ok(());
    }

    let display_options: Vec<&str> = entries.iter().map(|e| e.display.as_str()).collect();

    // Clear active_item_style so the per-row ANSI styling isn't overridden by
    // the selection highlight.
    let theme = ColorfulTheme {
        active_item_style: Style::new(),
        ..Default::default()
    };

    let term = Term::stdout();
    let selection = FuzzySelect::with_theme(&theme)
        .default(0)
        .highlight_matches(true)
        .items(&display_options)
        .interact_on(&term);

    match selection {
        Ok(index) => {
            let resolved = resolve_target(config, &entries[index].token);
            interactive_session(&resolved, detached, zzh_folder)?;
        }
        Err(_) => {
            let _ = Term::stderr().show_cursor();
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(toml_str: &str) -> Config {
        toml::from_str(toml_str).unwrap()
    }

    fn tokens(entries: &[MenuEntry]) -> Vec<&str> {
        entries.iter().map(|e| e.token.as_str()).collect()
    }

    #[test]
    fn menu_orders_history_then_config_then_ssh() {
        let config = cfg("[hosts.db]\nhostname = \"d\"\n");
        let history = vec![("zeta".to_string(), Utc::now())];
        let ssh_hosts = vec!["alpha".to_string()];
        let entries = assemble_menu(&history, &ssh_hosts, &config, None);
        assert_eq!(tokens(&entries), vec!["zeta", "db", "alpha"]);
    }

    #[test]
    fn group_menu_includes_unlisted_members() {
        // web1 is only a group member (no host entry, ssh-config, or history),
        // but `@prod` must still list it so the user can connect.
        let config = cfg("[hosts.db]\nhostname = \"d\"\n[groups]\nprod = [\"db\", \"web1\"]\n");
        let entries = assemble_menu(&[], &[], &config, Some("prod"));
        let tokens = tokens(&entries);
        assert!(tokens.contains(&"db"));
        assert!(tokens.contains(&"web1"));
    }

    #[test]
    fn group_menu_excludes_non_members() {
        let config = cfg(concat!(
            "[hosts.db]\nhostname = \"d\"\n",
            "[hosts.cache]\nhostname = \"c\"\n",
            "[groups]\nprod = [\"db\"]\n",
        ));
        let entries = assemble_menu(&[], &[], &config, Some("prod"));
        assert_eq!(tokens(&entries), vec!["db"]);
    }
}
