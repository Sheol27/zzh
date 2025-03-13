# ZZH - Improved SSH experience

ZZH is a cli tool that wraps SSH. It's not a drop in replacement, just a more enjoyable way of managing connections

## Features

- **Interactive Fuzzy Search:** Quickly find and connect to your hosts using a fuzzy search menu.
- **Connection History:** Automatically tracks your recent SSH connections for easy access.
- **Detached Sessions:** Option to start sessions in detached mode (no interactive shell).
- **SSH Config Integration:** Reads your existing SSH configuration from `~/.ssh/config` to list available hosts.
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

## How It Works

- **SSH Config Integration:**  
  ZZH locates and reads your SSH configuration from `$HOME/.ssh/config` to compile a list of available hosts.

- **Connection History:**  
  Each time you connect to a host, ZZH logs the connection with a timestamp in a history file (`~/.zzh/history`). This history is then used to prioritize frequently accessed hosts in the interactive menu.

- **Fuzzy Search Interface:**  
  The interactive menu allows you to search through your hosts with ease, displaying connection history with timestamps for added context.

## Configuration

- **History Storage:**  
  ZZH creates a `.zzh` folder in your home directory where it stores the connection history in a file named `history`. You can manually clear or edit this file as needed.

## Contributing

If you encounter any issues or have suggestions for improvements, please open an issue or submit a pull request.

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.

---

Enjoy a smoother SSH experience with ZZH!
