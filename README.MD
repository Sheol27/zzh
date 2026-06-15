# ZZH - Improved SSH experience

[![Crates.io](https://img.shields.io/crates/v/zzh)](https://crates.io/crates/zzh)

ZZH is a cli tool that wraps SSH. It's not a drop in replacement, just a more enjoyable way of managing connections

## Features

- **Interactive Fuzzy Search:** Quickly find and connect to your hosts using a fuzzy search menu.
- **Connection History:** Automatically tracks your recent SSH connections for easy access.
- **Detached Sessions:** Option to start sessions in detached mode (no interactive shell).
- **SSH Config Integration:** Reads your existing SSH configuration from `~/.ssh/config` to list available hosts.
- **TOML Config:** Define extra hosts, short aliases, and groups in `~/.zzh/config.toml` (see [Config file](#config-file)).
- **Shell Completions:** Tab completion support for bash, zsh, and fish with history-aware suggestions.
- **Simple & Fast:** Built with performance and usability in mind.

## Installation

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (for building from source)
- SSH installed and configured on your system

### Installation via Cargo

```bash
cargo install zzh
```

### Building from Source

1. **Clone the repository:**

   ```bash
   git clone https://github.com/yourusername/zzh.git
   cd zzh
   ```

2. **Build the project:**

   ```bash
   cargo build --release
   ```

3. **Run the binary:**

   The compiled binary will be available at `target/release/zzh`. You can run it directly or add it to your PATH for easier access.


## Usage

### Connect to a Host Directly

To connect to a specific host defined in your SSH configuration, simply pass the target host as an argument:

```bash
zzh <target-host>
```

### Interactive Host Selection

Run `zzh` without any arguments to launch an interactive fuzzy search menu. This interface combines your connection history and hosts from your SSH config, allowing you to select a host easily:

```bash
zzh
```

### Detached Sessions

If you want to establish a connection without opening an interactive shell (detached mode), use the `--detached` flag:

```bash
zzh <target-host> --detached
```

### List SSH Hosts

To list all hosts defined in your SSH configuration:

```bash
zzh --hosts
```

### Shell Completions

ZZH supports tab completion for bash, zsh, and fish shells. Completions suggest hosts from both your connection history (sorted by recency) and your SSH config.

To generate completions for your shell:

```bash
# Bash
zzh completions bash >> ~/.bashrc

# Zsh
zzh completions zsh >> ~/.zshrc

# Fish
zzh completions fish > ~/.config/fish/completions/zzh.fish
```

After adding the completions, restart your shell or source the config file to enable them.

## How It Works

- **SSH Config Integration:**  
  ZZH locates and reads your SSH configuration from `$HOME/.ssh/config` to compile a list of available hosts.

- **Connection History:**  
  Each time you connect to a host, ZZH logs the connection with a timestamp in a history file (`~/.zzh/history`). This history is then used to prioritize frequently accessed hosts in the interactive menu.

- **Fuzzy Search Interface:**  
  The interactive menu allows you to search through your hosts with ease, displaying connection history with timestamps for added context.

## Config file

ZZH reads `~/.zzh/config.toml` on every run (a commented template is written on
first launch). Use it to define hosts that aren't in `~/.ssh/config`, give long
targets short aliases, and organize hosts into groups. Everything defined here
shows up in the fuzzy menu, in `zzh --hosts`, and in shell completions.

```toml
# A host that isn't in ~/.ssh/config. Connect with `zzh db`.
[hosts.db]
hostname = "db.internal"                        # address ssh connects to
user = "admin"                                  # optional -> admin@db.internal
port = 2222                                     # optional -> -p 2222
identity_file = "~/.ssh/id_db"                  # optional -> -i (~ expanded)
proxy_jump = "bastion"                          # optional -> -J bastion
options = ["StrictHostKeyChecking=accept-new"]  # optional -> repeated -o

# Short names: `zzh w` connects to web1.
[aliases]
w = "web1"

# Named host groups: `zzh @prod` scopes the menu to these members.
[groups]
prod = ["web1", "web2", "db"]
```

Only `hosts.<name>` keys are required to make sense; every field is optional and
a host with no `hostname` falls back to its own name (so ssh still resolves it
via `~/.ssh/config`). Aliases resolve to a config host, an SSH-config host, or a
raw `user@host`.

### Editing from the command line

You can edit `config.toml` by hand, or let ZZH write it for you (comments and
formatting are preserved):

```bash
# Add or update a host
zzh add-host db --hostname db.internal --user admin --port 2222 \
  --identity ~/.ssh/id_db --jump bastion --option StrictHostKeyChecking=accept-new

# Define an alias
zzh alias d db

# Add hosts to a group (creates the group if needed)
zzh tag prod db web1 web2
```

### Using groups

```bash
# Open the fuzzy menu scoped to a single group
zzh @prod
```

## Configuration

- **Config file:**  
  Hosts, aliases, and groups live in `~/.zzh/config.toml`, see [Config file](#config-file) above.

- **History Storage:**  
  ZZH creates a `.zzh` folder in your home directory where it stores the connection history in a file named `history`. You can manually clear or edit this file as needed.

## Contributing

If you encounter any issues or have suggestions for improvements, please open an issue or submit a pull request.

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.

---

Enjoy a smoother SSH experience with ZZH!
