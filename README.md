# Aurum

Aurum is a lightweight keyboard-driven terminal dashboard for Arch Linux/CachyOS that integrates `paru` with package updates, AUR search, PKGBUILD security scanning, build cache inspection, and Arch news.

## Features

- **Asynchronous UI**: Built on Tokio and Ratatui. External commands and disk I/O run off the UI thread for smooth terminal interaction.
- **Paru integration**: View installed packages, available updates, orphaned packages, and install or upgrade directly from the TUI.
- **PKGBUILD security scanner**: Performs static analysis on PKGBUILD content and flags suspicious patterns like `curl | sh`, `eval`, `base64 -d`, and other risky operations.
- **Build cache inspection**: Scan `~/.cache/paru/clone`, inspect package build directories, and remove individual or all cached builds.
- **Application store**: Browse curated Arch/ AUR applications by category, view descriptions, and see installed status in one place.
- **Arch news feed**: Fetches official Arch Linux news to surface important announcements before package operations.
- **Clean shell suspension**: Restores the terminal before running external package commands and returns cleanly to the TUI afterwards.

## Installation

### Requirements

- Rust toolchain (`rustc` + `cargo`)
- `paru`
- `git`
- Arch-based distribution or compatible environment

### Install from repository

```bash
git clone https://github.com/NaveLIL/aurum.git
cd aurum
./install.sh
```

The install script builds the release binary, installs it to `~/.local/bin/aurum`, and installs the desktop entry to `~/.local/share/applications/aurum.desktop`.

Make sure `~/.local/bin` is included in your `PATH`.

## AUR packaging

Aurum includes a `PKGBUILD` and `.SRCINFO` for Arch/ AUR publication. If you want to build the package locally, use:

```bash
makepkg -si
```

This will build the binary and install the package into the local system package database.

## Usage

- Run `aurum` from a terminal
- Use `Tab` / `Shift+Tab` to switch tabs
- Use arrow keys or `j` / `k` to navigate lists
- Press `/` to search AUR
- Press `Enter` to view details or install a selected package
- Press `q` to quit

## Configuration

Aurum stores settings in `~/.config/aurum/config.json`. The file is created automatically on first launch.

Example configuration:

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

Add or adjust `risky_patterns` for your own security rules.

## Contributing

1. Fork the repository
2. Create a branch (`git checkout -b feature/name`)
3. Commit your changes (`git commit -m 'Add feature'`)
4. Push your branch (`git push origin feature/name`)
5. Open a pull request

## License

MIT
