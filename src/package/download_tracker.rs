use std::{
    fmt::Write,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use indicatif::{MultiProgress, ProgressBar, ProgressState, ProgressStyle};

use crate::core::{constant::SPARKLE, util::format_bytes};

pub struct DownloadTracker {
    total_bytes: AtomicU64,
    downloaded_bytes: AtomicU64,
    total_packages: usize,
    completed_packages: AtomicU64,
    total_progress_bar: ProgressBar,
}

impl DownloadTracker {
    pub fn new(
        total_packages: usize,
        total_bytes: u64,
        multi_progress: &MultiProgress,
    ) -> Arc<Self> {
        let total_progress_bar = multi_progress.insert(0, ProgressBar::new(total_bytes));

        total_progress_bar.set_style(
            ProgressStyle::with_template("{spinner} {spark}[{elapsed_precise}] {msg} ({eta})")
                .unwrap()
                .with_key("spark", |_: &ProgressState, w: &mut dyn Write| {
                    write!(w, "{SPARKLE}").unwrap()
                })
                .progress_chars("#-"),
        );

        Arc::new(Self {
            total_bytes: AtomicU64::new(total_bytes),
            downloaded_bytes: AtomicU64::new(0),
            total_packages,
            completed_packages: AtomicU64::new(0),
            total_progress_bar,
        })
    }

    pub fn update_progress_bar(&self) {
        self.total_progress_bar.set_message(format!(
            "Installed {}/{} packages, {}/{} downloaded",
            self.completed_packages.load(Ordering::Relaxed),
            self.total_packages,
            format_bytes(self.downloaded_bytes.load(Ordering::Relaxed)),
            format_bytes(self.total_bytes.load(Ordering::Relaxed)),
        ));
    }

    pub async fn add_downloaded_bytes(&self, bytes: u64) {
        self.downloaded_bytes.fetch_add(bytes, Ordering::Relaxed);
        self.update_progress_bar();
    }

    pub fn mark_package_completed(&self) {
        self.completed_packages.fetch_add(1, Ordering::Relaxed);
        self.update_progress_bar();
    }

    pub async fn finish_install(&self) {
        self.update_progress_bar();
        self.total_progress_bar.finish();
    }

    pub fn get_progress_bar(&self) -> &ProgressBar {
        &self.total_progress_bar
    }
}
