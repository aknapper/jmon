use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;

use anyhow::{Context, Result};
use regex::Regex;

use crate::model::{PowerRail, StatsSnapshot, TempReading};

pub struct TegrastatsRunner {
    rx: Receiver<StatsSnapshot>,
    child: Child,
}

impl TegrastatsRunner {
    pub fn spawn(path: &str, interval_ms: u64) -> Result<Self> {
        let mut child = Command::new(path)
            .arg("--interval")
            .arg(interval_ms.to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("failed to start tegrastats at `{}`", path))?;

        let stdout = child
            .stdout
            .take()
            .context("tegrastats stdout was not available")?;

        let (tx, rx) = mpsc::channel();
        let parser = TegrastatsParser::new();

        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().flatten() {
                if let Some(snapshot) = parser.parse_line(&line) {
                    let _ = tx.send(snapshot);
                }
            }
        });

        Ok(Self { rx, child })
    }

    pub fn try_recv(&self) -> Option<StatsSnapshot> {
        self.rx.try_recv().ok()
    }

    pub fn shutdown(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub struct TegrastatsParser {
    ram_re: Regex,
    swap_re: Regex,
    cpu_re: Regex,
    gpu_re: Regex,
    emc_re: Regex,
    temp_re: Regex,
    power_re: Regex,
}

impl TegrastatsParser {
    pub fn new() -> Self {
        Self {
            ram_re: Regex::new(r"RAM\s+(?P<used>\d+)/(?P<total>\d+)MB").unwrap(),
            swap_re: Regex::new(r"SWAP\s+(?P<used>\d+)/(?P<total>\d+)MB").unwrap(),
            cpu_re: Regex::new(r"CPU\s+\[(?P<list>[^\]]+)]").unwrap(),
            gpu_re: Regex::new(r"GR3D_FREQ\s+(?P<util>\d+)%").unwrap(),
            emc_re: Regex::new(r"EMC_FREQ\s+(?P<util>\d+)%").unwrap(),
            temp_re: Regex::new(r"(?P<name>[A-Za-z0-9_]+)@(?P<temp>\d+(?:\.\d+)?)C").unwrap(),
            power_re: Regex::new(
                r"(?P<name>[A-Z0-9_]+)\s+(?P<current>\d+)mW/(?P<avg>\d+)mW",
            )
            .unwrap(),
        }
    }

    pub fn parse_line(&self, line: &str) -> Option<StatsSnapshot> {
        let mut snapshot = StatsSnapshot::default();

        if let Some(caps) = self.ram_re.captures(line) {
            snapshot.ram_used_mb = caps.name("used").and_then(|v| v.as_str().parse().ok());
            snapshot.ram_total_mb = caps
                .name("total")
                .and_then(|v| v.as_str().parse().ok());
        }

        if let Some(caps) = self.swap_re.captures(line) {
            snapshot.swap_used_mb = caps.name("used").and_then(|v| v.as_str().parse().ok());
            snapshot.swap_total_mb = caps
                .name("total")
                .and_then(|v| v.as_str().parse().ok());
        }

        if let Some(caps) = self.cpu_re.captures(line) {
            if let Some(list) = caps.name("list") {
                snapshot.cpu_cores = parse_cpu_list(list.as_str());
            }
        }

        if let Some(caps) = self.gpu_re.captures(line) {
            snapshot.gpu_util = caps
                .name("util")
                .and_then(|v| v.as_str().parse::<f32>().ok());
        }

        if let Some(caps) = self.emc_re.captures(line) {
            snapshot.emc_util = caps
                .name("util")
                .and_then(|v| v.as_str().parse::<f32>().ok());
        }

        for caps in self.temp_re.captures_iter(line) {
            if let (Some(name), Some(temp)) = (caps.name("name"), caps.name("temp")) {
                if let Ok(value_c) = temp.as_str().parse::<f32>() {
                    snapshot.temps.push(TempReading {
                        name: name.as_str().to_string(),
                        value_c,
                    });
                }
            }
        }

        for caps in self.power_re.captures_iter(line) {
            if let (Some(name), Some(current), Some(avg)) = (
                caps.name("name"),
                caps.name("current"),
                caps.name("avg"),
            ) {
                if let (Ok(current_mw), Ok(average_mw)) =
                    (current.as_str().parse::<u64>(), avg.as_str().parse::<u64>())
                {
                    snapshot.power_rails.push(PowerRail {
                        name: name.as_str().to_string(),
                        current_mw,
                        average_mw,
                    });
                }
            }
        }

        let has_data = !snapshot.cpu_cores.is_empty()
            || snapshot.ram_used_mb.is_some()
            || snapshot.swap_used_mb.is_some()
            || snapshot.gpu_util.is_some()
            || snapshot.emc_util.is_some()
            || !snapshot.temps.is_empty()
            || !snapshot.power_rails.is_empty();

        if has_data {
            Some(snapshot)
        } else {
            None
        }
    }
}

fn parse_cpu_list(list: &str) -> Vec<f32> {
    list.split(',')
        .filter_map(|entry| {
            let trimmed = entry.trim();
            if trimmed.eq_ignore_ascii_case("off") {
                return Some(0.0);
            }
            let percent_part = trimmed.split('%').next().unwrap_or("");
            percent_part.trim().parse::<f32>().ok()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::TegrastatsParser;

    #[test]
    fn parses_sample_line() {
        let parser = TegrastatsParser::new();
        let line = "01-20-2026 22:46:22 RAM 17842/125772MB (lfb 79x4MB) CPU [0%@972,0%@972,0%@1566,0%@1566,0%@972,0%@972,0%@972,0%@972,0%@972,0%@972,1%@972,0%@972,0%@972,0%@972] cpu@30.468C tj@31.218C soc012@30.218C soc345@31.218C VDD_GPU 0mW/0mW VDD_CPU_SOC_MSS 5535mW/5535mW VIN_SYS_5V0 5040mW/5040mW VIN 16802mW/16802mW";
        let snapshot = parser.parse_line(line).expect("parse snapshot");

        assert_eq!(snapshot.ram_used_mb, Some(17842));
        assert_eq!(snapshot.ram_total_mb, Some(125772));
        assert_eq!(snapshot.cpu_cores.len(), 14);
        assert!(snapshot.power_rails.iter().any(|rail| rail.name == "VIN"));
        let vin = snapshot
            .power_rails
            .iter()
            .find(|rail| rail.name == "VIN")
            .expect("VIN rail");
        assert_eq!(vin.current_mw, 16802);
    }
}
