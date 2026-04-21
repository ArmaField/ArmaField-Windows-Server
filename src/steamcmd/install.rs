use std::fs;
use std::io::{self, Read};
use std::path::Path;

use tracing::info;

use crate::error::{Error, Result};

const STEAMCMD_URL: &str = "https://steamcdn-a.akamaihd.net/client/installer/steamcmd.zip";

pub fn ensure_steamcmd(steamcmd_dir: &Path, steamcmd_exe: &Path) -> Result<()> {
    if steamcmd_exe.exists() {
        return Ok(());
    }
    info!(target = %steamcmd_dir.display(), "downloading SteamCMD from {}", STEAMCMD_URL);
    fs::create_dir_all(steamcmd_dir)?;

    let resp = ureq::get(STEAMCMD_URL)
        .call()
        .map_err(|e| Error::Steamcmd(format!("download failed: {e}")))?;

    let mut buf = Vec::with_capacity(4 * 1024 * 1024);
    resp.into_reader()
        .take(64 * 1024 * 1024)
        .read_to_end(&mut buf)?;

    let cursor = io::Cursor::new(buf);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| Error::Steamcmd(format!("invalid zip: {e}")))?;
    archive
        .extract(steamcmd_dir)
        .map_err(|e| Error::Steamcmd(format!("extract failed: {e}")))?;

    if !steamcmd_exe.exists() {
        return Err(Error::Steamcmd(format!(
            "steamcmd.exe not found at {} after extraction",
            steamcmd_exe.display()
        )));
    }
    info!("SteamCMD installed at {}", steamcmd_exe.display());
    Ok(())
}
