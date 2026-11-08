//! Print jobs and their files on disk.

use crate::pdf::{Mode, layout, parse_pages};

pub const MAX_PAGES_PER_SIDE: usize = 100;
use lopdf::Document;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    io,
    path::PathBuf,
    time::{Duration, SystemTime},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Quality {
    Draft,
    #[default]
    Normal,
    High,
}

impl Quality {
    /// Translation key of the label.
    pub fn label_key(self) -> &'static str {
        match self {
            Quality::Draft => "q_draft",
            Quality::Normal => "q_normal",
            Quality::High => "q_high",
        }
    }

    /// IPP `print-quality` value.
    pub fn ipp(self) -> u8 {
        match self {
            Quality::Draft => 3,
            Quality::Normal => 4,
            Quality::High => 5,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Edge {
    #[default]
    Long,
    Short,
}

fn one() -> usize {
    1
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Job {
    pub printer: String,
    pub mode: Mode,
    pub pages: String,
    pub copies: u32,
    pub grayscale: bool,
    pub quality: Quality,
    edge: Edge,
    pub page_numbers: bool,
    #[serde(default = "one")]
    pages_per_side: usize,
    /// Printer test jobs are printed without the saved printer setting.
    pub test: bool,
}

impl Job {
    /// Builds a job from the upload form. Unknown values fall back to defaults.
    pub fn from_form(f: &HashMap<String, String>) -> Job {
        let get = |k: &str| f.get(k).map_or("", String::as_str);
        let checked = |k: &str| f.contains_key(k);
        Job {
            printer: get("printer").to_string(),
            mode: match get("mode") {
                "single" => Mode::Single,
                "booklet" => Mode::Booklet,
                _ => Mode::Duplex,
            },
            pages: get("pages").trim().to_string(),
            copies: get("copies").parse().unwrap_or(1).clamp(1, 99),
            grayscale: checked("gray"),
            quality: match get("quality") {
                "draft" => Quality::Draft,
                "high" => Quality::High,
                _ => Quality::Normal,
            },
            edge: if get("edge") == "short" {
                Edge::Short
            } else {
                Edge::Long
            },
            page_numbers: checked("numbers"),
            pages_per_side: get("nup").parse().unwrap_or(1).clamp(1, MAX_PAGES_PER_SIDE),
            test: false,
        }
    }

    pub fn printer_test(printer: String) -> Job {
        Job {
            printer,
            mode: Mode::Duplex,
            pages: String::new(),
            copies: 1,
            grayscale: false,
            quality: Quality::Normal,
            edge: Edge::Long,
            page_numbers: false,
            pages_per_side: 1,
            test: true,
        }
    }

    /// Booklets only fold correctly when the backs are turned over the short edge.
    pub fn edge(&self) -> Edge {
        if self.mode == Mode::Booklet {
            Edge::Short
        } else {
            self.edge
        }
    }

    /// Document pages on each side of a sheet.
    pub fn per_side(&self) -> usize {
        if self.mode == Mode::Booklet {
            2
        } else {
            self.pages_per_side
        }
    }

    /// Sheets of paper needed, copies included.
    pub fn sheets(&self, pages: &[usize]) -> usize {
        layout(self.mode, self.per_side(), pages).0.len() / self.per_side() * self.copies as usize
    }
}

/// Jobs are kept in one directory: `<id>-source.pdf`, `<id>-job.json` and the generated sides.
#[derive(Clone)]
pub struct JobStore {
    dir: PathBuf,
}

const FILES: [&str; 4] = ["source.pdf", "job.json", "front.pdf", "back.pdf"];
const MAX_AGE: Duration = Duration::from_secs(24 * 3600);

impl JobStore {
    pub fn new(dir: PathBuf) -> JobStore {
        JobStore { dir }
    }

    pub fn path(&self, id: &str, name: &str) -> PathBuf {
        self.dir.join(format!("{id}-{name}"))
    }

    pub async fn create(&self, source: &[u8], job: &Job) -> io::Result<String> {
        self.remove_stale().await;
        let id = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .to_string();
        tokio::fs::create_dir_all(&self.dir).await?;
        tokio::fs::write(self.path(&id, "source.pdf"), source).await?;
        tokio::fs::write(self.path(&id, "job.json"), serde_json::to_vec(job)?).await?;
        Ok(id)
    }

    /// The job and its selected pages, or `None` if the id is unknown.
    pub fn load(&self, id: &str) -> Option<(Job, Vec<usize>)> {
        if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let job: Job =
            serde_json::from_slice(&std::fs::read(self.path(id, "job.json")).ok()?).ok()?;
        let page_count = Document::load(self.path(id, "source.pdf"))
            .ok()?
            .get_pages()
            .len();
        let pages = parse_pages(&job.pages, page_count)?;
        Some((job, pages))
    }

    pub async fn remove(&self, id: &str) {
        for name in FILES {
            let _ = tokio::fs::remove_file(self.path(id, name)).await;
        }
    }

    /// Deletes files of jobs that were started but never finished.
    async fn remove_stale(&self) {
        let Ok(mut dir) = tokio::fs::read_dir(&self.dir).await else {
            return;
        };
        while let Ok(Some(entry)) = dir.next_entry().await {
            let modified = entry.metadata().await.and_then(|m| m.modified());
            if modified.is_ok_and(|t| t.elapsed().unwrap_or_default() > MAX_AGE) {
                let _ = tokio::fs::remove_file(entry.path()).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn booklet_always_flips_on_the_short_edge() {
        let job = Job::from_form(&form(&[("mode", "booklet"), ("edge", "long")]));
        assert_eq!(job.edge(), Edge::Short);
    }

    #[test]
    fn copies_are_clamped() {
        assert_eq!(Job::from_form(&form(&[("copies", "500")])).copies, 99);
        assert_eq!(Job::from_form(&form(&[("copies", "x")])).copies, 1);
    }

    #[test]
    fn custom_pages_per_side() {
        for n in [3, 4, 5, 6, 7, 12] {
            let job = Job::from_form(&form(&[("mode", "duplex"), ("nup", &n.to_string())]));
            assert_eq!(job.per_side(), n);
            assert_eq!(
                job.sheets(&(0..25).collect::<Vec<_>>()),
                25_usize.div_ceil(2 * n)
            );
        }
        assert_eq!(Job::from_form(&form(&[("nup", "0")])).per_side(), 1);
        assert_eq!(
            Job::from_form(&form(&[("nup", "101")])).per_side(),
            MAX_PAGES_PER_SIDE
        );
        assert_eq!(
            Job::from_form(&form(&[("mode", "booklet"), ("nup", "7")])).per_side(),
            2
        );
    }

    #[test]
    fn sheet_count_includes_copies() {
        let job = Job::from_form(&form(&[("mode", "duplex"), ("copies", "2")]));
        assert_eq!(job.sheets(&[0, 1, 2, 3, 4]), 6);
    }
}
