use crate::types::{Package, Update, ScanResult, NewsItem, CacheEntry, DiskStats, SystemInfo};
use crate::backend::flatpak::{FlatpakApp, FlatpakSearchApp};

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum Action {
    Tick,
    Quit,
    Resize(u16, u16),
    ToggleInputMode,
    SelectNext,
    SelectPrevious,
    ChangeTab(usize),
    // Data Loading
    SetInstalled(Vec<Package>),
    SetUpdates(Vec<Update>),
    SetSearchResults(Vec<Package>),
    SetScanResult(ScanResult),
    SetStatus(String),
    Error(String),
    Key(crossterm::event::KeyEvent),
    ScanPackage(String),
    ViewPkgbuild(String),
    SetPkgbuildLines(Vec<ratatui::text::Line<'static>>),
    // New: Package operations
    InstallPackages(Vec<String>),
    RemovePackages(Vec<String>),
    SetOrphans(Vec<Package>),
    ToggleSelect(String),
    UpdateAll,
    UpdateSingle(String),
    SetPackageInfo(Package),
    // New: News
    SetNews(Vec<NewsItem>),
    // New: Cache
    SetCacheEntries(Vec<CacheEntry>),
    CleanCache(String),
    CleanCacheSuccess(String),
    CleanAllCache,
    CleanAllCacheSuccess,
    // New: Confirm dialog
    ShowConfirm(String, Box<Action>),
    ConfirmYes,
    ConfirmNo,
    // Flatpak
    SetFlatpakAvailable(bool),
    SetFlatpakInstalled(Vec<FlatpakApp>),
    SetFlatpakSearchResults(Vec<FlatpakSearchApp>),
    InstallFlatpakTool,
    InstallFlatpakApp(String),
    RemoveFlatpakApp(String),
    // Disk Stats and Cleaners
    SetDiskStats(DiskStats),
    CleanPacmanCache(bool),
    CleanFlatpakUnused,
    ToggleHelp,
    // System Upgrade and Troubleshoot
    SetSystemInfo(SystemInfo),
    SystemUpgrade { use_snapper: bool },
    TroubleshootFixKeyring,
    TroubleshootResetKeys,
    TroubleshootRemoveLock,
    TroubleshootUpdateMirrors,
    TroubleshootInstallLtsKernel,
}

