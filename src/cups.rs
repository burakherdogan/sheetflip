//! Printing through CUPS (`lp`, `lpstat`) and the per-printer settings from the printer test.

use crate::{
    error::AppError,
    job::{Edge, Job, JobStore},
    pdf::{build_pdf, layout},
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, io, path::PathBuf};
use tokio::process::Command;

pub struct Printers {
    pub names: Vec<String>,
    pub default: Option<String>,
}

impl Printers {
    pub fn contains(&self, name: &str) -> bool {
        self.names.iter().any(|n| n == name)
    }
}

async fn lpstat(arg: &str) -> String {
    match Command::new("lpstat").arg(arg).output().await {
        Ok(out) => String::from_utf8_lossy(&out.stdout).into_owned(),
        Err(_) => String::new(),
    }
}

pub async fn printers() -> Printers {
    let names = lpstat("-e").await.lines().map(str::to_string).collect();
    let default = lpstat("-d")
        .await
        .rsplit_once(": ")
        .map(|(_, d)| d.trim().to_string());
    Printers { names, default }
}

async fn submit(id: &str, job: &Job, pdf: PathBuf) -> Result<(), AppError> {
    let mut cmd = Command::new("lp");
    cmd.args(["-d", &job.printer, "-n", &job.copies.to_string(), "-t", id])
        .args(["-o", "collate=true", "-o", "fit-to-page"])
        .args(["-o", &format!("print-quality={}", job.quality.ipp())]);
    cmd.args(["-o", "number-up=1"]);
    if job.grayscale {
        cmd.args(["-o", "print-color-mode=monochrome"]);
    }
    let out = cmd
        .arg(pdf)
        .output()
        .await
        .map_err(|e| AppError::Print(e.to_string()))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(AppError::Print(
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        ))
    }
}

/// How a printer's paper path affects the backs, found with the printer test.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PrinterSetting {
    /// The backs must be printed in reverse sheet order.
    pub reverse: bool,
    /// The backs come out upside down and must be turned by 180°.
    pub rotate: bool,
}

/// Printer settings, stored as JSON keyed by CUPS printer name.
#[derive(Clone)]
pub struct Settings {
    path: PathBuf,
}

impl Settings {
    pub fn new(path: PathBuf) -> Settings {
        Settings { path }
    }

    pub fn all(&self) -> BTreeMap<String, PrinterSetting> {
        std::fs::read(&self.path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, printer: &str, setting: PrinterSetting) -> io::Result<()> {
        let mut all = self.all();
        all.insert(printer.to_string(), setting);
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&self.path, serde_json::to_vec_pretty(&all)?)
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Side {
    Front,
    Back,
}

/// Generates one side of a job and sends it to the printer. Returns the job and its sheet count.
pub async fn print_side(
    jobs: &JobStore,
    settings: &Settings,
    id: &str,
    side: Side,
) -> Result<(Job, usize), AppError> {
    let (job, pages) = jobs.load(id).ok_or(AppError::JobNotFound)?;
    let (front, back) = layout(job.mode, job.per_side(), &pages);
    let order = if side == Side::Back { back } else { front };
    let mut sheets: Vec<Vec<Option<usize>>> =
        order.chunks(job.per_side()).map(<[_]>::to_vec).collect();

    // The reload steps on screen are always the same. The printer setting corrects for
    // the printer's paper path; a short-edge flip is made by turning the backs around.
    let setting = if job.test {
        PrinterSetting::default()
    } else {
        settings.all().remove(&job.printer).unwrap_or_default()
    };
    let back_side = side == Side::Back;
    let rotate = back_side && ((job.edge() == Edge::Short) != setting.rotate);
    if back_side && setting.reverse {
        sheets.reverse();
    }

    let out = jobs.path(id, if back_side { "back.pdf" } else { "front.pdf" });
    build_pdf(
        &jobs.path(id, "source.pdf"),
        &sheets.concat(),
        job.per_side(),
        rotate,
        job.page_numbers,
        &out,
    )?;
    submit(id, &job, out).await?;
    let count = job.sheets(&pages);
    Ok((job, count))
}
