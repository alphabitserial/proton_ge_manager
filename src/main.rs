mod app;
use app::ProtonGEManager;

use anyhow::Result;

fn main() -> Result<()> {
    let app = ProtonGEManager::default();
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(Box::new(app), native_options);
}
