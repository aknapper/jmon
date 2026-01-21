use std::env;
use std::io::{self, Write};
use std::thread;
use std::time::Duration;

const RAM_TOTAL_MB: u64 = 125_772;
const SWAP_TOTAL_MB: u64 = 8192;
const CPU_CORES: usize = 14;
const LFB_BLOCKS: u64 = 79;
const LFB_SIZE_MB: u64 = 4;

fn main() {
    let interval_ms = parse_interval_ms().unwrap_or(1000).clamp(100, 5000);
    let mut state = FakeState::new();

    loop {
        let line = state.next_line(interval_ms);
        println!("{}", line);
        let _ = io::stdout().flush();
        thread::sleep(Duration::from_millis(interval_ms));
    }
}

fn parse_interval_ms() -> Option<u64> {
    let mut args = env::args().skip(1);
    let mut interval = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--interval" | "-i" => {
                if let Some(value) = args.next() {
                    interval = value.parse::<u64>().ok();
                }
            }
            "--help" | "-h" => {
                println!("fake_tegrastats --interval <ms>");
                return None;
            }
            _ => {}
        }
    }

    interval
}

struct FakeState {
    tick: u64,
    carry_ms: u64,
    seed: u64,
    clock: FakeClock,
    avg_vdd_gpu: f64,
    avg_vdd_cpu: f64,
    avg_vin_sys: f64,
    avg_vin: f64,
}

impl FakeState {
    fn new() -> Self {
        Self {
            tick: 0,
            carry_ms: 0,
            seed: 0x5eeda5,
            clock: FakeClock::new(2026, 1, 20, 22, 46, 22),
            avg_vdd_gpu: 0.0,
            avg_vdd_cpu: 0.0,
            avg_vin_sys: 0.0,
            avg_vin: 0.0,
        }
    }

    fn next_line(&mut self, interval_ms: u64) -> String {
        let t = self.tick as f64 * interval_ms as f64 / 1000.0;

        let mut cpu_utils = Vec::with_capacity(CPU_CORES);
        for core in 0..CPU_CORES {
            let phase = core as f64 * 0.35;
            let base = wave(t, 0.6 + core as f64 * 0.02, phase, 2.0, 92.0);
            let util = (base + self.jitter(6.0)).clamp(0.0, 100.0);
            cpu_utils.push(util);
        }

        let cpu_total = cpu_utils.iter().sum::<f64>() / CPU_CORES as f64;

        let ram_used = (17842.0 + wave(t, 0.05, 0.0, -1800.0, 1800.0) + self.jitter(120.0))
            .clamp(8000.0, (RAM_TOTAL_MB - 1000) as f64)
            .round() as u64;

        let swap_used = (wave(t, 0.02, 0.5, 0.0, 256.0) + self.jitter(16.0))
            .clamp(0.0, SWAP_TOTAL_MB as f64)
            .round() as u64;

        let gpu_util = (wave(t, 0.35, 0.3, 5.0, 95.0) + self.jitter(4.0))
            .clamp(0.0, 100.0);
        let emc_util = (wave(t, 0.2, 1.1, 10.0, 90.0) + self.jitter(3.0))
            .clamp(0.0, 100.0);

        let cpu_temp = 30.0 + cpu_total * 0.45 + self.jitter(0.4);
        let tj_temp = cpu_temp + 1.0 + self.jitter(0.2);
        let soc012_temp = cpu_temp - 0.3 + self.jitter(0.2);
        let soc345_temp = cpu_temp + 0.4 + self.jitter(0.2);

        let vdd_gpu = (200.0 + gpu_util * 25.0 + self.jitter(40.0)).max(0.0);
        let vdd_cpu = (4800.0 + cpu_total * 40.0 + self.jitter(120.0)).max(0.0);
        let vin_sys = (4800.0 + wave(t, 0.1, 0.7, -200.0, 200.0) + self.jitter(60.0))
            .max(0.0);
        let overhead = 6000.0 + wave(t, 0.08, 0.2, -250.0, 250.0) + self.jitter(50.0);
        let vin = (vdd_gpu + vdd_cpu + vin_sys + overhead).max(0.0);

        let vdd_gpu_avg = smooth(&mut self.avg_vdd_gpu, vdd_gpu);
        let vdd_cpu_avg = smooth(&mut self.avg_vdd_cpu, vdd_cpu);
        let vin_sys_avg = smooth(&mut self.avg_vin_sys, vin_sys);
        let vin_avg = smooth(&mut self.avg_vin, vin);

        let cpu_list = cpu_utils
            .iter()
            .map(|util| {
                let freq = if *util > 70.0 { 1566 } else { 972 };
                format!("{}%@{}", util.round() as u64, freq)
            })
            .collect::<Vec<_>>()
            .join(",");

        let line = format!(
            "{} RAM {}/{}MB (lfb {}x{}MB) SWAP {}/{}MB CPU [{}] cpu@{:.3}C tj@{:.3}C soc012@{:.3}C soc345@{:.3}C VDD_GPU {}mW/{}mW VDD_CPU_SOC_MSS {}mW/{}mW VIN_SYS_5V0 {}mW/{}mW VIN {}mW/{}mW GR3D_FREQ {}% EMC_FREQ {}%",
            self.clock.format(),
            ram_used,
            RAM_TOTAL_MB,
            LFB_BLOCKS,
            LFB_SIZE_MB,
            swap_used,
            SWAP_TOTAL_MB,
            cpu_list,
            cpu_temp,
            tj_temp,
            soc012_temp,
            soc345_temp,
            vdd_gpu.round() as u64,
            vdd_gpu_avg,
            vdd_cpu.round() as u64,
            vdd_cpu_avg,
            vin_sys.round() as u64,
            vin_sys_avg,
            vin.round() as u64,
            vin_avg,
            gpu_util.round() as u64,
            emc_util.round() as u64
        );

        self.tick += 1;
        self.advance_clock(interval_ms);

        line
    }

    fn advance_clock(&mut self, interval_ms: u64) {
        self.carry_ms += interval_ms;
        while self.carry_ms >= 1000 {
            self.clock.tick();
            self.carry_ms -= 1000;
        }
    }

    fn jitter(&mut self, magnitude: f64) -> f64 {
        (self.next_unit() * 2.0 - 1.0) * magnitude
    }

    fn next_unit(&mut self) -> f64 {
        self.seed = self.seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let value = (self.seed >> 32) as u32;
        value as f64 / u32::MAX as f64
    }
}

fn smooth(avg: &mut f64, value: f64) -> u64 {
    if *avg == 0.0 {
        *avg = value;
    } else {
        *avg = *avg * 0.85 + value * 0.15;
    }
    avg.round() as u64
}

fn wave(t: f64, freq: f64, phase: f64, min: f64, max: f64) -> f64 {
    let range = max - min;
    let value = (t * freq + phase).sin() * 0.5 + 0.5;
    min + value * range
}

struct FakeClock {
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
}

impl FakeClock {
    fn new(year: i32, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> Self {
        Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
        }
    }

    fn tick(&mut self) {
        self.second += 1;
        if self.second >= 60 {
            self.second = 0;
            self.minute += 1;
        }
        if self.minute >= 60 {
            self.minute = 0;
            self.hour += 1;
        }
        if self.hour >= 24 {
            self.hour = 0;
            self.day += 1;
        }
        if self.day > 28 {
            self.day = 1;
            self.month += 1;
        }
        if self.month > 12 {
            self.month = 1;
            self.year += 1;
        }
    }

    fn format(&self) -> String {
        format!(
            "{:02}-{:02}-{:04} {:02}:{:02}:{:02}",
            self.month, self.day, self.year, self.hour, self.minute, self.second
        )
    }
}
