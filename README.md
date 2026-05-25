# Aurum

A lightweight, keyboard-driven terminal dashboard and security auditor for the Arch Linux/CachyOS AUR helper (`paru`). Built with Rust, Ratatui, and Tokio.

Aurum integrates package updates, official news, clone directories inspection, and static analysis of PKGBUILD files into a single asynchronous command-line workspace.

![Aurum Screen Preview](https://raw.githubusercontent.com/username/aurum/main/assets/preview.png) *(Add your screenshot here)*

## Features

- **Asynchronous Engine**: Built on Tokio. All external commands (invoking `paru`, fetching metadata) and recursive disk I/O are performed out-of-thread to ensure the UI frame rate remains perfectly fluid.
- **PKGBUILD Security Scan**: Statically inspects package recipes (PKGBUILD) for suspicious code patterns—such as Base64 decoding, nested `eval`, remote scripts piped to `sh`, or dangerous file deletions—before compilation.
- **Cache Pruning**: Scans and displays exact build cache size inside `~/.cache/paru/clone`. Prune single package builds or purge everything in one keystroke.
- **Arch News Feed**: RSS parsing of the official Arch Linux feeds to alert you to manual packaging interventions before running updates.
- **Clean Shell Suspension**: Smoothly drops back to the raw terminal layout during compilation and package installation, returning focus back to the TUI once the subshell closes.

## Installation

Ensure you have `paru` and a working Rust toolchain.

Clone the repository and run the installation script:

```bash
git clone https://github.com/yourusername/aurum.git
cd aurum
./install.sh
```

The script compiles the project in release mode, installs the binary at `~/.local/bin/aurum`, and copies the desktop file launcher to your local share applications directory. Make sure `~/.local/bin` is in your `$PATH`.

## Default Keybindings

| Key | Action |
| --- | --- |
| `Tab` / `Shift+Tab` | Navigate between dashboard tabs |
| `j` / `Down` / `k` / `Up` | Scroll through lists (Updates, Installed, News, Cache, Scanner) |
| `/` | Focus the AUR search bar |
| `Enter` | Install selected package (in Updates/Search) or View Details (in Installed) |
| `s` | Run security analysis on selected PKGBUILD |
| `u` | Update currently selected package |
| `U` | Run full upgrade of all AUR packages (`paru -Sua`) |
| `d` | Delete selected clone build directory from cache |
| `D` | Clear all build directories in the clone cache |
| `Esc` | Cancel input focus / exit search |
| `q` | Quit |

## Configuration

Aurum stores its settings in `~/.config/aurum/config.json`. The configuration file is generated automatically on the first launch.

Example structure:

```json
{
  "check_interval_minutes": 60,
  "aur_rpc_url": "https://aur.archlinux.org/rpc/",
  "max_cache_size_mb": 5000,
  "risky_patterns": [
    "rm\\s+-rf\\s+.*",
    "curl\\s+.*\\|\\s*sh",
    "wget\\s+.*\\|\\s*sh",
    "eval\\s+",
    "base64\\s+-d",
    "sudo\\s+"
  ],
  "theme": "default"
}
```

You can add custom regular expressions to `risky_patterns` to expand the static analysis scanner for your specific rules.

## Contributing

1. Fork it
2. Create your feature branch (`git checkout -b feature/cool-idea`)
3. Commit your changes (`git commit -am 'Add cool-idea'`)
4. Push to the branch (`git push origin feature/cool-idea`)
5. Create a new Pull Request

## License

MIT
