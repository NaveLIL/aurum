# Aurum

Aurum is a lightweight, keyboard-driven terminal dashboard and package management tool for Arch Linux and CachyOS. It integrates `paru` and `pacman` with system health monitoring, upgrade safety nets, automatic diagnostics, and a smart command-line installer.

## Features

- **Asynchronous UI**: Built on Tokio and Ratatui. All heavy operations (news fetching, package database queries, disk scans) run on background threads to keep the UI responsive.
- **Smart CLI Universal Installer**: 
  - `aurum install <package>`: Installs packages from official repositories or the AUR.
  - `aurum install <path/to/archive.pkg.tar.zst>`: Automatically installs local package files via `pacman -U`.
  - `aurum install <path/to/PKGBUILD_dir>`: Automatically runs `makepkg -si` to build and install packages from source directories.
  - `aurum remove <packages...>`: Cleanly uninstalls packages and their unused dependencies via `paru -Rns`.
  - *Safety Guard*: Detects and prevents building packages (`makepkg`) as root, guiding the user to run without `sudo`.
- **Upgrade Safety Net (Snapper)**:
  - Detects Btrfs `snapper` configurations.
  - Automatically creates matched atomic `pre` and `post` snapshot restore points before and after running system upgrades (`paru -Syu`).
- **System Health Diagnostics**:
  - **LTS Backup Kernel Checker**: Scans for installed LTS kernels. Displays warnings on the Dashboard if no backup kernel is configured to prevent unbootable systems during kernel upgrades.
  - **LTS Kernel Installer**: Press `Shift-B` to install the appropriate LTS kernel and headers (`linux-cachyos-lts` or `linux-lts` + headers) dynamically matching your active distribution.
  - **Large Cache Warning**: Flags pacman package cache if it exceeds 5 GB.
  - **Low Disk Warning**: Alerts the user if root `/` free space drops below 10 GB.
- **Troubleshooting Toolkit**:
  - `[K]` Fix Arch Keyring and refresh signature keys (`sudo pacman -Sy archlinux-keyring && sudo pacman-key --refresh-keys`).
  - `[L]` Remove stale pacman database locks (`/var/lib/pacman/db.lck`).
  - `[R]` Re-initialize pacman keys database (`sudo pacman-key --init && sudo pacman-key --populate archlinux`).
  - `[M]` Update mirrorlist dynamically using `reflector` to find the 20 fastest HTTPS mirrors, then sync package databases (`sudo pacman -Sy`).
- **Layout Independent Hotkeys**: Automatically translates Cyrillic (Russian) keyboard layout inputs to Latin equivalents so all TUI shortcuts work regardless of active keyboard layout.
- **PKGBUILD Security Scanner**: Statically analyzes package PKGBUILDs to flag risky patterns like `curl | sh`, `eval`, `base64 -d`, and other command injections.
- **News Feed**: Displays recent Arch Linux news items on the Dashboard, warning you of manual intervention alerts before system upgrades.

## Installation

### Requirements

- Rust toolchain (`rustc` + `cargo`)
- `paru`
- `git`
- Arch-based distribution (Arch, CachyOS, EndeavourOS, etc.)

### Install from source

```bash
git clone https://github.com/NaveLIL/aurum.git
cd aurum
./install.sh
```

The installer builds the release binary, copies it to `~/.local/bin/aurum`, and installs the desktop launcher to `~/.local/share/applications/aurum.desktop`.

Ensure `~/.local/bin` is in your `PATH`.

To build the local package database entry using `makepkg`:
```bash
makepkg -si
```

## Usage

### Command Line Interface

```bash
aurum                                 # Launch TUI dashboard
aurum install <pkg | file | dir>      # Smart install
aurum remove <packages...>            # Clean uninstall
aurum help, -h, --help                # Show usage help
```

### TUI Navigation

- **Tab / ]** / **Shift-Tab / [**: Switch tabs
- **1 - 8**: Switch directly to tab 1-8
- **j / k / Arrow keys**: Navigate lists
- **/**: Search packages
- **Enter**: View package details / install
- **u / U**: Upgrade selected package / Full system upgrade
- **?**: Toggle keyboard shortcuts modal
- **Esc / q**: Close modals / Quit

### Troubleshooting Shortcuts (Normal Mode)

- **Shift-K**: Repair GPG Keyrings
- **Shift-L**: Delete pacman `db.lck` database lock
- **Shift-R**: Re-initialize and populate pacman key database
- **Shift-M**: Sort and update fast mirrors list using Reflector
- **Shift-B**: Install safety backup LTS kernel and headers

## Configuration

Settings are stored in `~/.config/aurum/config.json` (created automatically on first launch).

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

## License

MIT
