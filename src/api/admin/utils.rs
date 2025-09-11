use anyhow::{Context, Result, anyhow};
use itertools::Itertools; // optional, 也可不用
use sea_orm::*;
use std::{
    collections::HashSet,
    io::{Read, Seek},
    path::{Path, PathBuf},
};
use tokio::task;

#[derive(Debug, serde::Serialize)]
pub struct ImportResult {
    pub inserted: Vec<challenges::Model>,
    pub skipped_by_name: Vec<String>,
    pub failed: Vec<(String, String)>, // (name or file-stem, error)
}

fn meta_name_from_toml(meta: &str) -> Result<String> {
    let c = cm::ChallengeMeta::from_toml_str(meta).context("parse meta.toml failed")?;
    Ok(c.name)
}

fn sanitize_zip_relpath<'a>(file: &zip::read::ZipFile<'a>) -> Result<PathBuf> {
    // 优先使用 enclosed_name（或 sanitized_name，依 zip 版本）
    let rel = file
        .enclosed_name()
        .ok_or_else(|| anyhow!("zip entry has invalid or unsafe path"))?;
    Ok(rel.to_path_buf())
}

/// 仅从 zip 里读出 meta.toml 内容（不解压）
fn read_meta_from_zip<R: Read + Seek>(archive: &mut zip::ZipArchive<R>) -> Result<String> {
    let mut f = archive
        .by_name("meta.toml")
        .context("meta.toml not found in zip")?;
    let mut buf = String::new();
    f.read_to_string(&mut buf)?;
    Ok(buf)
}

/// 把 zip 解压到 output_dir（已做 Zip Slip 防护）
fn extract_zip_to<R: Read + Seek>(
    mut archive: zip::ZipArchive<R>,
    output_dir: &Path,
) -> Result<()> {
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let rel = sanitize_zip_relpath(&file)?;
        let out_path = output_dir.join(rel);

        if !out_path.starts_with(output_dir) {
            anyhow::bail!("zip entry escapes output dir");
        }

        if file.name().ends_with('/') || file.name().ends_with('\\') {
            std::fs::create_dir_all(&out_path)?;
            continue;
        }

        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut outfile = std::fs::File::create(&out_path)?;
        std::io::copy(&mut file, &mut outfile)?;
    }
    Ok(())
}
