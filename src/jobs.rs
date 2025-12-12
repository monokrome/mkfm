use std::path::PathBuf;
use std::time::Instant;
use tokio::sync::mpsc;

pub type JobId = u64;

#[derive(Clone, Debug)]
pub enum JobStatus {
    Pending,
    Running,
    Complete,
    Failed(String),
}

#[derive(Clone, Debug)]
pub enum JobKind {
    Copy { src: PathBuf, dest: PathBuf },
    Move { src: PathBuf, dest: PathBuf },
    Trash { path: PathBuf },
    Extract { archive: PathBuf, dest: PathBuf },
}

#[derive(Clone, Debug)]
pub struct Job {
    pub id: JobId,
    pub kind: JobKind,
    pub description: String,
    pub status: JobStatus,
    pub progress: Option<f32>,  // 0.0-1.0 for measurable, None for spinner
    pub created_at: Instant,
    pub completed_at: Option<Instant>,
}

impl Job {
    pub fn new(id: JobId, kind: JobKind) -> Self {
        let description = match &kind {
            JobKind::Copy { src, dest } => {
                let src_name = src.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                let dest_name = dest.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                format!("Copy {} -> {}", src_name, dest_name)
            }
            JobKind::Move { src, dest } => {
                let src_name = src.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                let dest_name = dest.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                format!("Move {} -> {}", src_name, dest_name)
            }
            JobKind::Trash { path } => {
                let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                format!("Trash {}", name)
            }
            JobKind::Extract { archive, .. } => {
                let name = archive.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                format!("Extract {}", name)
            }
        };
        Self {
            id,
            kind,
            description,
            status: JobStatus::Pending,
            progress: None,
            created_at: Instant::now(),
            completed_at: None,
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self.status, JobStatus::Pending | JobStatus::Running)
    }

    pub fn is_failed(&self) -> bool {
        matches!(self.status, JobStatus::Failed(_))
    }

    pub fn is_complete(&self) -> bool {
        matches!(self.status, JobStatus::Complete)
    }
}

/// Messages from background tasks to UI
#[derive(Debug)]
pub enum JobUpdate {
    Started(JobId),
    Progress(JobId, f32),
    Complete(JobId),
    Failed(JobId, String),
}

/// The job queue manager
pub struct JobQueue {
    jobs: Vec<Job>,
    next_id: JobId,
    update_rx: mpsc::Receiver<JobUpdate>,
    update_tx: mpsc::Sender<JobUpdate>,
}

impl JobQueue {
    pub fn new() -> Self {
        let (update_tx, update_rx) = mpsc::channel(64);
        Self {
            jobs: Vec::new(),
            next_id: 0,
            update_rx,
            update_tx,
        }
    }

    pub fn submit(&mut self, kind: JobKind) -> JobId {
        let id = self.next_id;
        self.next_id += 1;
        let job = Job::new(id, kind);
        self.jobs.push(job);
        id
    }

    pub fn get(&self, id: JobId) -> Option<&Job> {
        self.jobs.iter().find(|j| j.id == id)
    }

    pub fn active_jobs(&self) -> impl Iterator<Item = &Job> {
        self.jobs.iter().filter(|j| j.is_active())
    }

    pub fn completed_jobs(&self) -> impl Iterator<Item = &Job> {
        self.jobs.iter().filter(|j| j.is_complete())
    }

    pub fn failed_jobs(&self) -> impl Iterator<Item = &Job> {
        self.jobs.iter().filter(|j| j.is_failed())
    }

    pub fn has_active_jobs(&self) -> bool {
        self.jobs.iter().any(|j| j.is_active())
    }

    pub fn active_count(&self) -> usize {
        self.jobs.iter().filter(|j| j.is_active()).count()
    }

    pub fn failed_count(&self) -> usize {
        self.jobs.iter().filter(|j| j.is_failed()).count()
    }

    /// Non-blocking poll for updates
    pub fn poll_updates(&mut self) {
        while let Ok(update) = self.update_rx.try_recv() {
            self.apply_update(update);
        }
    }

    fn apply_update(&mut self, update: JobUpdate) {
        match update {
            JobUpdate::Started(id) => {
                if let Some(job) = self.jobs.iter_mut().find(|j| j.id == id) {
                    job.status = JobStatus::Running;
                }
            }
            JobUpdate::Progress(id, progress) => {
                if let Some(job) = self.jobs.iter_mut().find(|j| j.id == id) {
                    job.progress = Some(progress);
                }
            }
            JobUpdate::Complete(id) => {
                if let Some(job) = self.jobs.iter_mut().find(|j| j.id == id) {
                    job.status = JobStatus::Complete;
                    job.progress = Some(1.0);
                    job.completed_at = Some(Instant::now());
                }
            }
            JobUpdate::Failed(id, msg) => {
                if let Some(job) = self.jobs.iter_mut().find(|j| j.id == id) {
                    job.status = JobStatus::Failed(msg);
                    job.completed_at = Some(Instant::now());
                }
            }
        }
    }

    pub fn sender(&self) -> mpsc::Sender<JobUpdate> {
        self.update_tx.clone()
    }

    /// Clear completed jobs older than the given duration
    pub fn clear_completed(&mut self, max_age_secs: u64) {
        let now = Instant::now();
        self.jobs.retain(|job| {
            if let Some(completed) = job.completed_at {
                if job.is_complete() && now.duration_since(completed).as_secs() > max_age_secs {
                    return false;
                }
            }
            true
        });
    }

    /// Get all jobs for display
    pub fn all_jobs(&self) -> &[Job] {
        &self.jobs
    }
}

// ==================== Async Job Executor ====================

use crate::filesystem;

pub async fn execute_job(id: JobId, kind: JobKind, tx: mpsc::Sender<JobUpdate>) {
    let _ = tx.send(JobUpdate::Started(id)).await;

    let result = match kind {
        JobKind::Copy { src, dest } => copy_with_progress(&src, &dest, id, &tx).await,
        JobKind::Move { src, dest } => move_file(&src, &dest).await,
        JobKind::Trash { path } => trash_file(&path).await,
        JobKind::Extract { archive, dest } => extract_archive(&archive, &dest).await,
    };

    match result {
        Ok(()) => { let _ = tx.send(JobUpdate::Complete(id)).await; }
        Err(e) => { let _ = tx.send(JobUpdate::Failed(id, e.to_string())).await; }
    }
}

async fn copy_with_progress(
    src: &PathBuf,
    dest: &PathBuf,
    id: JobId,
    tx: &mpsc::Sender<JobUpdate>,
) -> std::io::Result<()> {
    use tokio::fs;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    if src.is_dir() {
        // Directory copy - use blocking operation
        let src = src.clone();
        let dest = dest.clone();
        tokio::task::spawn_blocking(move || filesystem::copy_file(&src, &dest))
            .await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))??;
        Ok(())
    } else {
        // File copy with byte-level progress
        let metadata = fs::metadata(src).await?;
        let total_bytes = metadata.len();

        let mut src_file = fs::File::open(src).await?;
        let mut dest_file = fs::File::create(dest).await?;

        let mut copied = 0u64;
        let mut buffer = vec![0u8; 64 * 1024];  // 64KB buffer
        let mut last_progress = 0.0f32;

        loop {
            let n = src_file.read(&mut buffer).await?;
            if n == 0 { break; }

            dest_file.write_all(&buffer[..n]).await?;
            copied += n as u64;

            // Only send progress updates when significant change (>1%)
            let progress = if total_bytes > 0 { copied as f32 / total_bytes as f32 } else { 1.0 };
            if progress - last_progress > 0.01 {
                let _ = tx.send(JobUpdate::Progress(id, progress)).await;
                last_progress = progress;
            }
        }

        Ok(())
    }
}

async fn move_file(src: &PathBuf, dest: &PathBuf) -> std::io::Result<()> {
    // Try rename first (fast, same filesystem)
    match tokio::fs::rename(src, dest).await {
        Ok(()) => Ok(()),
        Err(_) => {
            // Fall back to copy + delete (cross-filesystem)
            let src = src.clone();
            let dest = dest.clone();
            tokio::task::spawn_blocking(move || {
                filesystem::copy_file(&src, &dest)?;
                filesystem::delete(&src)
            })
            .await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
        }
    }
}

async fn trash_file(path: &PathBuf) -> std::io::Result<()> {
    let path = path.clone();
    tokio::task::spawn_blocking(move || filesystem::trash(&path))
        .await
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
}

async fn extract_archive(archive: &PathBuf, dest: &PathBuf) -> std::io::Result<()> {
    let archive = archive.clone();
    let dest = dest.clone();
    tokio::task::spawn_blocking(move || filesystem::extract_archive(&archive, &dest))
        .await
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
}

// ==================== Task List and Error List Panes ====================

pub struct TaskListPane {
    pub cursor: usize,
    pub visible: bool,
    pub scroll_offset: usize,
}

impl TaskListPane {
    pub fn new() -> Self {
        Self {
            cursor: 0,
            visible: false,
            scroll_offset: 0,
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn show(&mut self) {
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }
}

pub struct ErrorListPane {
    pub cursor: usize,
    pub visible: bool,
    pub scroll_offset: usize,
}

impl ErrorListPane {
    pub fn new() -> Self {
        Self {
            cursor: 0,
            visible: false,
            scroll_offset: 0,
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn show(&mut self) {
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }
}
