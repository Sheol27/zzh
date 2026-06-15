use chrono::prelude::*;
use std::collections::HashMap;
use std::error::Error;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

const HISTORY_FILE: &str = "history";

/// `(host, last-connected timestamp)` pairs, newest first.
pub type HistoryEntries = Vec<(String, DateTime<Utc>)>;

pub fn extract_history(history_folder: &Path) -> Result<HistoryEntries, Box<dyn Error>> {
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

    // Keep only the most recent timestamp per host.
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

    let mut sorted_vec: HistoryEntries = latest_entries
        .into_iter()
        .map(|(key, datetime)| (key.to_string(), datetime))
        .collect();
    sorted_vec.sort_by_key(|entry| std::cmp::Reverse(entry.1));

    Ok(sorted_vec)
}

pub fn append_to_history(history_folder: &Path, host: &str) -> Result<(), Box<dyn Error>> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(history_folder.join(HISTORY_FILE))
        .unwrap();

    let dt_now = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    if let Err(e) = writeln!(file, "{dt_now} {host}") {
        eprintln!("Couldn't write to file: {e}");
    }

    Ok(())
}
