//! Adaptive Concurrency Control for PDF Generation
//!
//! This module provides intelligent resource-aware concurrency management:
//! - ResourceMonitor: Real-time CPU/RAM monitoring
//! - JobClassifier: Estimates resource requirements per job
//! - AdaptiveSemaphore: Dynamic concurrency based on available resources
//! - ConcurrencyController: Orchestrates all components

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tracing::{debug, error, info, warn};

// ============================================
// Configuration
// ============================================

/// Configuration for the concurrency controller
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConcurrencyConfig {
    /// Maximum concurrent PDF jobs (heavy - Typst compilation)
    pub max_pdf_concurrent: usize,
    /// Maximum concurrent Excel jobs (medium)
    pub max_excel_concurrent: usize,
    /// Maximum concurrent CSV jobs (light)
    pub max_csv_concurrent: usize,
    /// Total maximum concurrent jobs across all types
    pub max_total_concurrent: usize,
    /// RAM threshold (percentage) to start rejecting new jobs
    pub ram_threshold_percent: u8,
    /// CPU load threshold (1-minute avg) to start rejecting new jobs
    pub load_threshold: f64,
    /// Minimum free RAM in MB to accept new large jobs
    pub min_free_ram_mb: u64,
    /// Resource check interval in milliseconds
    pub resource_check_interval_ms: u64,
    /// Enable adaptive concurrency based on resource usage
    pub adaptive_enabled: bool,
    /// Job timeout in seconds (default 5 minutes)
    pub job_timeout_secs: u64,
    /// Queue size limit per job type
    pub max_queue_size: usize,
}

impl Default for ConcurrencyConfig {
    fn default() -> Self {
        Self {
            max_pdf_concurrent: 2,     // Heavy jobs - limit to 2
            max_excel_concurrent: 4,   // Medium jobs
            max_csv_concurrent: 8,     // Light jobs
            max_total_concurrent: 6,   // Total limit
            ram_threshold_percent: 85, // Start rejecting at 85% RAM
            load_threshold: 2.0,       // Max load as multiplier of cores (2.0 = 200% CPU)
            min_free_ram_mb: 1024,     // Need at least 1GB free for large jobs
            resource_check_interval_ms: 1000,
            adaptive_enabled: true,
            job_timeout_secs: 300, // 5 minutes
            max_queue_size: 100,
        }
    }
}

impl ConcurrencyConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        let mut config = Self::default();

        if let Ok(v) = std::env::var("CONCURRENCY_MAX_PDF") {
            config.max_pdf_concurrent = v.parse().unwrap_or(config.max_pdf_concurrent);
        }
        if let Ok(v) = std::env::var("CONCURRENCY_MAX_EXCEL") {
            config.max_excel_concurrent = v.parse().unwrap_or(config.max_excel_concurrent);
        }
        if let Ok(v) = std::env::var("CONCURRENCY_MAX_CSV") {
            config.max_csv_concurrent = v.parse().unwrap_or(config.max_csv_concurrent);
        }
        if let Ok(v) = std::env::var("CONCURRENCY_MAX_TOTAL") {
            config.max_total_concurrent = v.parse().unwrap_or(config.max_total_concurrent);
        }
        if let Ok(v) = std::env::var("CONCURRENCY_RAM_THRESHOLD") {
            config.ram_threshold_percent = v.parse().unwrap_or(config.ram_threshold_percent);
        }
        if let Ok(v) = std::env::var("CONCURRENCY_LOAD_THRESHOLD") {
            config.load_threshold = v.parse().unwrap_or(config.load_threshold);
        }
        if let Ok(v) = std::env::var("CONCURRENCY_MIN_FREE_RAM_MB") {
            config.min_free_ram_mb = v.parse().unwrap_or(config.min_free_ram_mb);
        }
        if let Ok(v) = std::env::var("CONCURRENCY_ADAPTIVE_ENABLED") {
            config.adaptive_enabled = v.parse().unwrap_or(config.adaptive_enabled);
        }
        if let Ok(v) = std::env::var("CONCURRENCY_JOB_TIMEOUT_SECS") {
            config.job_timeout_secs = v.parse().unwrap_or(config.job_timeout_secs);
        }

        config
    }
}

// ============================================
// Resource Monitoring
// ============================================

/// System resource snapshot
#[derive(Debug, Clone)]
pub struct ResourceSnapshot {
    /// Total RAM in bytes
    pub total_ram_bytes: u64,
    /// Used RAM in bytes
    pub used_ram_bytes: u64,
    /// Free RAM in bytes
    pub free_ram_bytes: u64,
    /// RAM usage percentage (0-100)
    pub ram_percent: u8,
    /// CPU load average (1 minute)
    pub load_1min: f64,
    /// CPU load average (5 minutes)
    pub load_5min: f64,
    /// Number of CPU cores
    pub cpu_cores: usize,
    /// Timestamp of the snapshot
    pub timestamp: Instant,
}

impl Default for ResourceSnapshot {
    fn default() -> Self {
        Self {
            total_ram_bytes: 0,
            used_ram_bytes: 0,
            free_ram_bytes: 0,
            ram_percent: 0,
            load_1min: 0.0,
            load_5min: 0.0,
            cpu_cores: 1,
            timestamp: Instant::now(),
        }
    }
}

impl ResourceSnapshot {
    /// Check if resources are critically low
    /// Note: load_threshold is interpreted as a multiplier of CPU cores
    /// e.g., load_threshold=2.0 on a 4-core machine means max load of 8.0
    pub fn is_critical(&self, config: &ConcurrencyConfig) -> bool {
        let max_load = config.load_threshold * self.cpu_cores.max(1) as f64;
        self.ram_percent >= config.ram_threshold_percent || self.load_1min >= max_load
    }

    /// Check if there's enough RAM for a large job
    pub fn has_ram_for_large_job(&self, config: &ConcurrencyConfig) -> bool {
        let free_mb = self.free_ram_bytes / (1024 * 1024);
        free_mb >= config.min_free_ram_mb
    }

    /// Get a health score (0-100, higher is better)
    pub fn health_score(&self) -> u8 {
        let ram_score = 100u8.saturating_sub(self.ram_percent);
        let load_per_core = self.load_1min / self.cpu_cores.max(1) as f64;
        let load_score = (100.0 - (load_per_core * 50.0).min(100.0)) as u8;
        (ram_score + load_score) / 2
    }
}

/// Resource monitor that periodically checks system resources
pub struct ResourceMonitor {
    snapshot: parking_lot::RwLock<ResourceSnapshot>,
    config: ConcurrencyConfig,
}

impl ResourceMonitor {
    /// Create a new resource monitor
    pub fn new(config: ConcurrencyConfig) -> Self {
        Self {
            snapshot: parking_lot::RwLock::new(ResourceSnapshot::default()),
            config,
        }
    }

    /// Get the current resource snapshot
    pub fn snapshot(&self) -> ResourceSnapshot {
        self.snapshot.read().clone()
    }

    /// Update the resource snapshot (called periodically)
    pub fn update(&self) {
        let snapshot = self.collect_resources();
        *self.snapshot.write() = snapshot;
    }

    /// Check if system resources allow accepting a new job
    pub fn can_accept_job(&self, job_size: JobSize) -> ResourceDecision {
        let snapshot = self.snapshot();

        // Check critical thresholds
        if snapshot.is_critical(&self.config) {
            return ResourceDecision::Reject {
                reason: format!(
                    "System overloaded: RAM {}%, Load {:.2}",
                    snapshot.ram_percent, snapshot.load_1min
                ),
            };
        }

        // For large jobs, check minimum RAM requirement
        if job_size == JobSize::Large && !snapshot.has_ram_for_large_job(&self.config) {
            return ResourceDecision::Reject {
                reason: format!(
                    "Insufficient RAM for large job: {} MB free, need {} MB",
                    snapshot.free_ram_bytes / (1024 * 1024),
                    self.config.min_free_ram_mb
                ),
            };
        }

        // Check if we should throttle based on load
        if snapshot.load_1min > self.config.load_threshold * 0.8 {
            return ResourceDecision::Throttle {
                delay_ms: 1000,
                reason: format!("High load: {:.2}", snapshot.load_1min),
            };
        }

        ResourceDecision::Accept {
            health_score: snapshot.health_score(),
        }
    }

    /// Collect current system resources
    #[cfg(target_os = "linux")]
    fn collect_resources(&self) -> ResourceSnapshot {
        let mut snapshot = ResourceSnapshot {
            timestamp: Instant::now(),
            cpu_cores: num_cpus_cached(),
            ..Default::default()
        };

        // Read /proc/meminfo for RAM
        if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
            for line in content.lines() {
                if line.starts_with("MemTotal:") {
                    snapshot.total_ram_bytes = parse_meminfo_kb(line) * 1024;
                } else if line.starts_with("MemAvailable:") {
                    snapshot.free_ram_bytes = parse_meminfo_kb(line) * 1024;
                }
            }
            snapshot.used_ram_bytes = snapshot
                .total_ram_bytes
                .saturating_sub(snapshot.free_ram_bytes);
            if snapshot.total_ram_bytes > 0 {
                snapshot.ram_percent = ((snapshot.used_ram_bytes as f64
                    / snapshot.total_ram_bytes as f64)
                    * 100.0) as u8;
            }
        }

        // Read /proc/loadavg for CPU load
        if let Ok(content) = std::fs::read_to_string("/proc/loadavg") {
            let parts: Vec<&str> = content.split_whitespace().collect();
            if parts.len() >= 2 {
                snapshot.load_1min = parts[0].parse().unwrap_or(0.0);
                snapshot.load_5min = parts[1].parse().unwrap_or(0.0);
            }
        }

        snapshot
    }

    #[cfg(target_os = "macos")]
    fn collect_resources(&self) -> ResourceSnapshot {
        let mut snapshot = ResourceSnapshot {
            timestamp: Instant::now(),
            cpu_cores: num_cpus_cached(),
            ..Default::default()
        };

        // Use sysctl for memory info on macOS
        if let Ok(output) = std::process::Command::new("vm_stat").output() {
            if let Ok(content) = String::from_utf8(output.stdout) {
                let page_size: u64 = 4096; // macOS uses 4KB pages
                let mut pages_free = 0u64;
                let mut pages_active = 0u64;
                let mut pages_inactive = 0u64;
                let mut pages_wired = 0u64;
                let mut pages_speculative = 0u64;

                for line in content.lines() {
                    if line.contains("Pages free:") {
                        pages_free = parse_vm_stat_value(line);
                    } else if line.contains("Pages active:") {
                        pages_active = parse_vm_stat_value(line);
                    } else if line.contains("Pages inactive:") {
                        pages_inactive = parse_vm_stat_value(line);
                    } else if line.contains("Pages wired down:") {
                        pages_wired = parse_vm_stat_value(line);
                    } else if line.contains("Pages speculative:") {
                        pages_speculative = parse_vm_stat_value(line);
                    }
                }

                // Total physical memory (approximate from sysctl)
                if let Ok(output) = std::process::Command::new("sysctl")
                    .args(["-n", "hw.memsize"])
                    .output()
                {
                    if let Ok(mem_str) = String::from_utf8(output.stdout) {
                        snapshot.total_ram_bytes = mem_str.trim().parse().unwrap_or(0);
                    }
                }

                snapshot.free_ram_bytes =
                    (pages_free + pages_inactive + pages_speculative) * page_size;
                snapshot.used_ram_bytes = (pages_active + pages_wired) * page_size;

                if snapshot.total_ram_bytes > 0 {
                    snapshot.ram_percent = ((snapshot.used_ram_bytes as f64
                        / snapshot.total_ram_bytes as f64)
                        * 100.0) as u8;
                }
            }
        }

        // Get load average using sysctl
        if let Ok(output) = std::process::Command::new("sysctl")
            .args(["-n", "vm.loadavg"])
            .output()
        {
            if let Ok(content) = String::from_utf8(output.stdout) {
                // Format: { 1.23 4.56 7.89 }
                let content = content.trim().trim_matches(|c| c == '{' || c == '}');
                let parts: Vec<&str> = content.split_whitespace().collect();
                if parts.len() >= 2 {
                    snapshot.load_1min = parts[0].parse().unwrap_or(0.0);
                    snapshot.load_5min = parts[1].parse().unwrap_or(0.0);
                }
            }
        }

        snapshot
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    fn collect_resources(&self) -> ResourceSnapshot {
        // Fallback for other platforms - return safe defaults
        ResourceSnapshot {
            timestamp: Instant::now(),
            cpu_cores: num_cpus_cached(),
            total_ram_bytes: 8 * 1024 * 1024 * 1024, // Assume 8GB
            free_ram_bytes: 4 * 1024 * 1024 * 1024,  // Assume 4GB free
            used_ram_bytes: 4 * 1024 * 1024 * 1024,
            ram_percent: 50,
            load_1min: 1.0,
            load_5min: 1.0,
        }
    }
}

/// Parse KB value from /proc/meminfo line
#[cfg(target_os = "linux")]
fn parse_meminfo_kb(line: &str) -> u64 {
    line.split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

/// Parse value from vm_stat line on macOS
#[cfg(target_os = "macos")]
fn parse_vm_stat_value(line: &str) -> u64 {
    line.split(':')
        .nth(1)
        .map(|s| s.trim().trim_end_matches('.'))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

/// Get cached CPU count
fn num_cpus_cached() -> usize {
    static CPU_COUNT: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *CPU_COUNT.get_or_init(|| {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(2)
    })
}

/// Resource decision
#[derive(Debug, Clone)]
pub enum ResourceDecision {
    Accept { health_score: u8 },
    Throttle { delay_ms: u64, reason: String },
    Reject { reason: String },
}

// ============================================
// Job Classification
// ============================================

/// Job size classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum JobSize {
    Small,  // < 500 records
    Medium, // 500-5000 records
    Large,  // > 5000 records
}

impl JobSize {
    /// Classify based on record count
    pub fn from_record_count(count: usize) -> Self {
        if count < 500 {
            JobSize::Small
        } else if count < 5000 {
            JobSize::Medium
        } else {
            JobSize::Large
        }
    }

    /// Estimated RAM usage in MB
    pub fn estimated_ram_mb(&self) -> u64 {
        match self {
            JobSize::Small => 256,
            JobSize::Medium => 768,
            JobSize::Large => 2048,
        }
    }

    /// Priority (lower = higher priority)
    pub fn priority(&self) -> u8 {
        match self {
            JobSize::Small => 1, // Highest priority - finish quick jobs first
            JobSize::Medium => 2,
            JobSize::Large => 3,
        }
    }
}

/// Job type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum JobType {
    Pdf,
    Excel,
    Csv,
}

impl JobType {
    /// Get the weight of this job type (higher = more resource intensive)
    pub fn weight(&self) -> u8 {
        match self {
            JobType::Pdf => 3,   // Heavy - spawns Typst process
            JobType::Excel => 2, // Medium - in-process
            JobType::Csv => 1,   // Light
        }
    }
}

/// Complete job classification
#[derive(Debug, Clone)]
pub struct JobClassification {
    pub job_type: JobType,
    pub job_size: JobSize,
    pub record_count: usize,
    pub estimated_ram_mb: u64,
    pub priority: u8,
    pub weight: u8,
}

impl JobClassification {
    /// Create a new job classification
    pub fn new(job_type: JobType, record_count: usize) -> Self {
        let job_size = JobSize::from_record_count(record_count);
        let base_ram = job_size.estimated_ram_mb();

        // Adjust RAM estimate based on job type
        let estimated_ram_mb = match job_type {
            JobType::Pdf => base_ram * 2, // Typst uses more RAM
            JobType::Excel => base_ram,
            JobType::Csv => base_ram / 2,
        };

        Self {
            job_type,
            job_size,
            record_count,
            estimated_ram_mb,
            priority: job_size.priority(),
            weight: job_type.weight(),
        }
    }

    /// Composite score for resource requirement (higher = more resources needed)
    pub fn resource_score(&self) -> u16 {
        (self.weight as u16) * 100 + (self.estimated_ram_mb / 100) as u16
    }
}

// ============================================
// Adaptive Semaphore
// ============================================

/// Semaphore that adjusts permits based on resource availability
pub struct AdaptiveSemaphore {
    /// Per-type semaphores
    pdf_semaphore: Arc<Semaphore>,
    excel_semaphore: Arc<Semaphore>,
    csv_semaphore: Arc<Semaphore>,
    /// Global semaphore for total concurrency
    global_semaphore: Arc<Semaphore>,
    /// Configuration
    config: ConcurrencyConfig,
    /// Active job counts (wrapped in Arc for sharing with permits)
    active_pdf: Arc<AtomicUsize>,
    active_excel: Arc<AtomicUsize>,
    active_csv: Arc<AtomicUsize>,
    /// Queue depths
    queued_pdf: AtomicUsize,
    queued_excel: AtomicUsize,
    queued_csv: AtomicUsize,
}

impl AdaptiveSemaphore {
    /// Create a new adaptive semaphore
    pub fn new(config: ConcurrencyConfig) -> Self {
        Self {
            pdf_semaphore: Arc::new(Semaphore::new(config.max_pdf_concurrent)),
            excel_semaphore: Arc::new(Semaphore::new(config.max_excel_concurrent)),
            csv_semaphore: Arc::new(Semaphore::new(config.max_csv_concurrent)),
            global_semaphore: Arc::new(Semaphore::new(config.max_total_concurrent)),
            config,
            active_pdf: Arc::new(AtomicUsize::new(0)),
            active_excel: Arc::new(AtomicUsize::new(0)),
            active_csv: Arc::new(AtomicUsize::new(0)),
            queued_pdf: AtomicUsize::new(0),
            queued_excel: AtomicUsize::new(0),
            queued_csv: AtomicUsize::new(0),
        }
    }

    /// Try to acquire a permit for a job type
    pub async fn acquire(&self, job_type: JobType) -> Result<JobPermit, ConcurrencyError> {
        // Increment queue counter
        self.increment_queue(job_type);

        // Get the type-specific semaphore
        let type_semaphore = match job_type {
            JobType::Pdf => &self.pdf_semaphore,
            JobType::Excel => &self.excel_semaphore,
            JobType::Csv => &self.csv_semaphore,
        };

        // Set timeout for acquiring permits
        let timeout = Duration::from_secs(self.config.job_timeout_secs);

        // Acquire global permit first
        let global_permit = match tokio::time::timeout(
            timeout,
            self.global_semaphore.clone().acquire_owned(),
        )
        .await
        {
            Ok(Ok(permit)) => permit,
            Ok(Err(_)) => {
                self.decrement_queue(job_type);
                return Err(ConcurrencyError::SemaphoreClosed);
            }
            Err(_) => {
                self.decrement_queue(job_type);
                return Err(ConcurrencyError::Timeout);
            }
        };

        // Acquire type-specific permit
        let type_permit =
            match tokio::time::timeout(timeout, type_semaphore.clone().acquire_owned()).await {
                Ok(Ok(permit)) => permit,
                Ok(Err(_)) => {
                    self.decrement_queue(job_type);
                    drop(global_permit);
                    return Err(ConcurrencyError::SemaphoreClosed);
                }
                Err(_) => {
                    self.decrement_queue(job_type);
                    drop(global_permit);
                    return Err(ConcurrencyError::Timeout);
                }
            };

        // Decrement queue, increment active
        self.decrement_queue(job_type);
        self.increment_active(job_type);

        // Create callback to decrement active counter on drop
        let active_counter = self.get_active_counter_arc(job_type);
        let on_drop = Box::new(move || {
            active_counter.fetch_sub(1, Ordering::SeqCst);
        });

        Ok(JobPermit {
            _global_permit: global_permit,
            _type_permit: type_permit,
            job_type,
            acquired_at: Instant::now(),
            on_drop: Some(on_drop),
        })
    }

    /// Try to acquire immediately (non-blocking)
    pub fn try_acquire(&self, job_type: JobType) -> Result<JobPermit, ConcurrencyError> {
        let type_semaphore = match job_type {
            JobType::Pdf => &self.pdf_semaphore,
            JobType::Excel => &self.excel_semaphore,
            JobType::Csv => &self.csv_semaphore,
        };

        // Try global permit
        let global_permit = self
            .global_semaphore
            .clone()
            .try_acquire_owned()
            .map_err(|_| ConcurrencyError::WouldBlock)?;

        // Try type permit
        let type_permit = match type_semaphore.clone().try_acquire_owned() {
            Ok(permit) => permit,
            Err(_) => {
                drop(global_permit);
                return Err(ConcurrencyError::WouldBlock);
            }
        };

        self.increment_active(job_type);

        // Create callback to decrement active counter on drop
        let active_counter = self.get_active_counter_arc(job_type);
        let on_drop = Box::new(move || {
            active_counter.fetch_sub(1, Ordering::SeqCst);
        });

        Ok(JobPermit {
            _global_permit: global_permit,
            _type_permit: type_permit,
            job_type,
            acquired_at: Instant::now(),
            on_drop: Some(on_drop),
        })
    }

    fn increment_queue(&self, job_type: JobType) {
        match job_type {
            JobType::Pdf => self.queued_pdf.fetch_add(1, Ordering::SeqCst),
            JobType::Excel => self.queued_excel.fetch_add(1, Ordering::SeqCst),
            JobType::Csv => self.queued_csv.fetch_add(1, Ordering::SeqCst),
        };
    }

    fn decrement_queue(&self, job_type: JobType) {
        match job_type {
            JobType::Pdf => self.queued_pdf.fetch_sub(1, Ordering::SeqCst),
            JobType::Excel => self.queued_excel.fetch_sub(1, Ordering::SeqCst),
            JobType::Csv => self.queued_csv.fetch_sub(1, Ordering::SeqCst),
        };
    }

    fn increment_active(&self, job_type: JobType) {
        match job_type {
            JobType::Pdf => self.active_pdf.fetch_add(1, Ordering::SeqCst),
            JobType::Excel => self.active_excel.fetch_add(1, Ordering::SeqCst),
            JobType::Csv => self.active_csv.fetch_add(1, Ordering::SeqCst),
        };
    }

    fn get_active_counter_arc(&self, job_type: JobType) -> Arc<AtomicUsize> {
        match job_type {
            JobType::Pdf => Arc::clone(&self.active_pdf),
            JobType::Excel => Arc::clone(&self.active_excel),
            JobType::Csv => Arc::clone(&self.active_csv),
        }
    }

    /// Get current statistics
    pub fn stats(&self) -> SemaphoreStats {
        SemaphoreStats {
            active_pdf: self.active_pdf.load(Ordering::SeqCst),
            active_excel: self.active_excel.load(Ordering::SeqCst),
            active_csv: self.active_csv.load(Ordering::SeqCst),
            queued_pdf: self.queued_pdf.load(Ordering::SeqCst),
            queued_excel: self.queued_excel.load(Ordering::SeqCst),
            queued_csv: self.queued_csv.load(Ordering::SeqCst),
            max_pdf: self.config.max_pdf_concurrent,
            max_excel: self.config.max_excel_concurrent,
            max_csv: self.config.max_csv_concurrent,
            max_total: self.config.max_total_concurrent,
        }
    }
}

/// Permit that must be held while processing a job
pub struct JobPermit {
    _global_permit: OwnedSemaphorePermit,
    _type_permit: OwnedSemaphorePermit,
    job_type: JobType,
    acquired_at: Instant,
    /// Callback to decrement active counter on drop
    on_drop: Option<Box<dyn FnOnce() + Send + Sync>>,
}

impl Drop for JobPermit {
    fn drop(&mut self) {
        // Call the on_drop callback to decrement the counter
        if let Some(callback) = self.on_drop.take() {
            callback();
        }

        let duration = self.acquired_at.elapsed();
        debug!(
            "Job permit released: type={:?}, duration={:?}",
            self.job_type, duration
        );
    }
}

/// Semaphore statistics
#[derive(Debug, Clone, Serialize)]
pub struct SemaphoreStats {
    pub active_pdf: usize,
    pub active_excel: usize,
    pub active_csv: usize,
    pub queued_pdf: usize,
    pub queued_excel: usize,
    pub queued_csv: usize,
    pub max_pdf: usize,
    pub max_excel: usize,
    pub max_csv: usize,
    pub max_total: usize,
}

impl SemaphoreStats {
    pub fn total_active(&self) -> usize {
        self.active_pdf + self.active_excel + self.active_csv
    }

    pub fn total_queued(&self) -> usize {
        self.queued_pdf + self.queued_excel + self.queued_csv
    }
}

// ============================================
// Concurrency Errors
// ============================================

#[derive(Debug, Clone, thiserror::Error)]
pub enum ConcurrencyError {
    #[error("System resources exhausted: {reason}")]
    ResourceExhausted { reason: String },

    #[error("Queue full for job type: {job_type:?}")]
    QueueFull { job_type: JobType },

    #[error("Timeout waiting for permit")]
    Timeout,

    #[error("Would block - no permits available")]
    WouldBlock,

    #[error("Semaphore closed")]
    SemaphoreClosed,

    #[error("Job rejected due to backpressure")]
    Backpressure,
}

// ============================================
// Concurrency Controller
// ============================================

/// Main controller that orchestrates resource monitoring and job scheduling
pub struct ConcurrencyController {
    config: ConcurrencyConfig,
    resource_monitor: Arc<ResourceMonitor>,
    semaphore: Arc<AdaptiveSemaphore>,
    /// Job metrics
    jobs_accepted: AtomicU64,
    jobs_rejected: AtomicU64,
    jobs_completed: AtomicU64,
    jobs_failed: AtomicU64,
    /// Active jobs tracking for timeout detection
    active_jobs: DashMap<String, JobInfo>,
}

/// Information about an active job
#[derive(Debug, Clone)]
pub struct JobInfo {
    pub job_id: String,
    pub classification: JobClassification,
    pub started_at: Instant,
    pub tenant_id: u64,
}

impl ConcurrencyController {
    /// Create a new concurrency controller
    pub fn new(config: ConcurrencyConfig) -> Self {
        let resource_monitor = Arc::new(ResourceMonitor::new(config.clone()));
        let semaphore = Arc::new(AdaptiveSemaphore::new(config.clone()));

        Self {
            config,
            resource_monitor,
            semaphore,
            jobs_accepted: AtomicU64::new(0),
            jobs_rejected: AtomicU64::new(0),
            jobs_completed: AtomicU64::new(0),
            jobs_failed: AtomicU64::new(0),
            active_jobs: DashMap::new(),
        }
    }

    /// Create with default configuration from environment
    pub fn from_env() -> Self {
        Self::new(ConcurrencyConfig::from_env())
    }

    /// Start the background resource monitoring task
    pub fn start_monitor(&self) -> tokio::task::JoinHandle<()> {
        let monitor = self.resource_monitor.clone();
        let interval_ms = self.config.resource_check_interval_ms;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(interval_ms));
            loop {
                interval.tick().await;
                monitor.update();
            }
        })
    }

    /// Try to acquire a permit for a job
    pub async fn acquire_permit(
        &self,
        job_id: &str,
        classification: JobClassification,
        tenant_id: u64,
    ) -> Result<JobPermit, ConcurrencyError> {
        // Check resource availability if adaptive mode is enabled
        if self.config.adaptive_enabled {
            match self
                .resource_monitor
                .can_accept_job(classification.job_size)
            {
                ResourceDecision::Accept { health_score } => {
                    debug!(
                        "Resource check passed: health_score={}, job_id={}",
                        health_score, job_id
                    );
                }
                ResourceDecision::Throttle { delay_ms, reason } => {
                    warn!(
                        "Throttling job {}: {} (delay {}ms)",
                        job_id, reason, delay_ms
                    );
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                }
                ResourceDecision::Reject { reason } => {
                    self.jobs_rejected.fetch_add(1, Ordering::SeqCst);
                    error!("Rejecting job {}: {}", job_id, reason);
                    return Err(ConcurrencyError::ResourceExhausted { reason });
                }
            }
        }

        // Check queue depth
        let stats = self.semaphore.stats();
        let queued = match classification.job_type {
            JobType::Pdf => stats.queued_pdf,
            JobType::Excel => stats.queued_excel,
            JobType::Csv => stats.queued_csv,
        };

        if queued >= self.config.max_queue_size {
            self.jobs_rejected.fetch_add(1, Ordering::SeqCst);
            return Err(ConcurrencyError::QueueFull {
                job_type: classification.job_type,
            });
        }

        // Acquire permit
        let permit = self.semaphore.acquire(classification.job_type).await?;

        // Track active job
        self.active_jobs.insert(
            job_id.to_string(),
            JobInfo {
                job_id: job_id.to_string(),
                classification,
                started_at: Instant::now(),
                tenant_id,
            },
        );

        self.jobs_accepted.fetch_add(1, Ordering::SeqCst);

        info!(
            "Job {} acquired permit: type={:?}, active={}, queued={}",
            job_id,
            permit.job_type,
            stats.total_active() + 1,
            stats.total_queued()
        );

        Ok(permit)
    }

    /// Mark a job as completed
    pub fn complete_job(&self, job_id: &str, success: bool) {
        if let Some((_, job_info)) = self.active_jobs.remove(job_id) {
            let duration = job_info.started_at.elapsed();

            if success {
                self.jobs_completed.fetch_add(1, Ordering::SeqCst);
                info!("Job {} completed successfully in {:?}", job_id, duration);
            } else {
                self.jobs_failed.fetch_add(1, Ordering::SeqCst);
                warn!("Job {} failed after {:?}", job_id, duration);
            }
        }
    }

    /// Get current resource snapshot
    pub fn resources(&self) -> ResourceSnapshot {
        self.resource_monitor.snapshot()
    }

    /// Get semaphore statistics
    pub fn semaphore_stats(&self) -> SemaphoreStats {
        self.semaphore.stats()
    }

    /// Get controller metrics
    pub fn metrics(&self) -> ControllerMetrics {
        let resources = self.resources();
        let semaphore = self.semaphore_stats();

        ControllerMetrics {
            jobs_accepted: self.jobs_accepted.load(Ordering::SeqCst),
            jobs_rejected: self.jobs_rejected.load(Ordering::SeqCst),
            jobs_completed: self.jobs_completed.load(Ordering::SeqCst),
            jobs_failed: self.jobs_failed.load(Ordering::SeqCst),
            active_jobs: self.active_jobs.len(),
            ram_percent: resources.ram_percent,
            load_1min: resources.load_1min,
            health_score: resources.health_score(),
            semaphore,
        }
    }

    /// Check if system is healthy enough to accept new jobs
    pub fn is_healthy(&self) -> bool {
        let resources = self.resources();
        !resources.is_critical(&self.config)
    }

    /// Get configuration
    pub fn config(&self) -> &ConcurrencyConfig {
        &self.config
    }
}

/// Controller metrics for monitoring
#[derive(Debug, Clone, Serialize)]
pub struct ControllerMetrics {
    pub jobs_accepted: u64,
    pub jobs_rejected: u64,
    pub jobs_completed: u64,
    pub jobs_failed: u64,
    pub active_jobs: usize,
    pub ram_percent: u8,
    pub load_1min: f64,
    pub health_score: u8,
    pub semaphore: SemaphoreStats,
}

// ============================================
// Tests
// ============================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_classification() {
        let small = JobClassification::new(JobType::Pdf, 100);
        assert_eq!(small.job_size, JobSize::Small);
        assert_eq!(small.priority, 1);

        let medium = JobClassification::new(JobType::Excel, 2000);
        assert_eq!(medium.job_size, JobSize::Medium);
        assert_eq!(medium.priority, 2);

        let large = JobClassification::new(JobType::Csv, 10000);
        assert_eq!(large.job_size, JobSize::Large);
        assert_eq!(large.priority, 3);
    }

    #[test]
    fn test_config_from_env() {
        std::env::set_var("CONCURRENCY_MAX_PDF", "5");
        let config = ConcurrencyConfig::from_env();
        assert_eq!(config.max_pdf_concurrent, 5);
        std::env::remove_var("CONCURRENCY_MAX_PDF");
    }

    #[tokio::test]
    async fn test_semaphore_basic() {
        let config = ConcurrencyConfig {
            max_pdf_concurrent: 2,
            max_total_concurrent: 3,
            ..Default::default()
        };
        let semaphore = AdaptiveSemaphore::new(config);

        // Acquire first permit
        let permit1 = semaphore.try_acquire(JobType::Pdf);
        assert!(permit1.is_ok());

        // Acquire second permit
        let permit2 = semaphore.try_acquire(JobType::Pdf);
        assert!(permit2.is_ok());

        // Third should fail (max_pdf_concurrent = 2)
        let permit3 = semaphore.try_acquire(JobType::Pdf);
        assert!(permit3.is_err());

        // But CSV should work (different type)
        let permit4 = semaphore.try_acquire(JobType::Csv);
        assert!(permit4.is_ok());

        // Fourth should fail (max_total_concurrent = 3)
        let permit5 = semaphore.try_acquire(JobType::Csv);
        assert!(permit5.is_err());
    }

    #[test]
    fn test_resource_snapshot_health() {
        let snapshot = ResourceSnapshot {
            ram_percent: 50,
            load_1min: 1.0,
            cpu_cores: 2,
            ..Default::default()
        };

        // Health score should be reasonable
        let score = snapshot.health_score();
        assert!(score > 0 && score <= 100);
    }
}
