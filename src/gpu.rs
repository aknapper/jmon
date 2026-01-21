use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

use anyhow::{Context, Result};

pub struct GpuUtilRunner {
    rx: mpsc::Receiver<f32>,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl GpuUtilRunner {
    pub fn spawn(path: &str, interval_ms: u64) -> Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = Arc::clone(&stop);
        let (tx, rx) = mpsc::channel();
        let path = path.to_string();

        query_gpu_util(&path).context("nvidia-smi not available")?;

        let handle = thread::spawn(move || {
            while !stop_thread.load(Ordering::Relaxed) {
                if let Ok(Some(util)) = query_gpu_util(&path) {
                    let _ = tx.send(util);
                }
                thread::sleep(Duration::from_millis(interval_ms));
            }
        });

        Ok(Self {
            rx,
            stop,
            handle: Some(handle),
        })
    }

    pub fn try_recv(&self) -> Option<f32> {
        self.rx.try_recv().ok()
    }

    pub fn shutdown(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn query_gpu_util(path: &str) -> Result<Option<f32>> {
    let output = Command::new(path)
        .arg("--query-gpu=utilization.gpu")
        .arg("--format=csv,noheader,nounits")
        .output()
        .context("failed to run nvidia-smi")?;

    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "nvidia-smi returned exit code {}",
            output.status
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let values: Vec<f32> = stdout
        .lines()
        .filter_map(|line| line.trim().parse::<f32>().ok())
        .collect();

    if values.is_empty() {
        Ok(None)
    } else {
        let avg = values.iter().sum::<f32>() / values.len() as f32;
        Ok(Some(avg))
    }
}
