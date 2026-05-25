use crate::types::{Package, Update, ScanResult, NewsItem, CacheEntry};

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
    // New: Package operations
    InstallPackages(Vec<String>),
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
}
