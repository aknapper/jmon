use std::collections::VecDeque;

#[derive(Clone, Debug, Default)]
pub struct StatsSnapshot {
    pub cpu_cores: Vec<f32>,
    pub ram_used_mb: Option<u64>,
    pub ram_total_mb: Option<u64>,
    pub swap_used_mb: Option<u64>,
    pub swap_total_mb: Option<u64>,
    pub gpu_util: Option<f32>,
    pub emc_util: Option<f32>,
    pub temps: Vec<TempReading>,
    pub power_rails: Vec<PowerRail>,
}

impl StatsSnapshot {
    pub fn cpu_total(&self) -> Option<f32> {
        if self.cpu_cores.is_empty() {
            None
        } else {
            Some(self.cpu_cores.iter().sum::<f32>() / self.cpu_cores.len() as f32)
        }
    }

    pub fn ram_percent(&self) -> Option<f32> {
        match (self.ram_used_mb, self.ram_total_mb) {
            (Some(used), Some(total)) if total > 0 => {
                Some((used as f32 / total as f32) * 100.0)
            }
            _ => None,
        }
    }

    pub fn total_power_mw(&self) -> Option<u64> {
        if self.power_rails.is_empty() {
            return None;
        }

        let mut sum_non_vin = 0;
        let mut has_non_vin = false;

        for rail in &self.power_rails {
            if rail.name == "VIN" {
                continue;
            }
            has_non_vin = true;
            sum_non_vin += rail.current_mw;
        }

        if has_non_vin {
            Some(sum_non_vin)
        } else {
            Some(self.power_rails.iter().map(|rail| rail.current_mw).sum())
        }
    }
}

#[derive(Clone, Debug)]
pub struct PowerRail {
    pub name: String,
    pub current_mw: u64,
    pub average_mw: u64,
}

#[derive(Clone, Debug)]
pub struct TempReading {
    pub name: String,
    pub value_c: f32,
}

#[derive(Debug)]
pub struct History {
    capacity: usize,
    pub cpu_total: VecDeque<u64>,
    pub ram_used: VecDeque<u64>,
    pub gpu_util: VecDeque<u64>,
    pub power_total: VecDeque<u64>,
}

impl History {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            cpu_total: VecDeque::with_capacity(capacity),
            ram_used: VecDeque::with_capacity(capacity),
            gpu_util: VecDeque::with_capacity(capacity),
            power_total: VecDeque::with_capacity(capacity),
        }
    }

    pub fn reset(&mut self) {
        self.cpu_total.clear();
        self.ram_used.clear();
        self.gpu_util.clear();
        self.power_total.clear();
    }

    pub fn push(&mut self, snapshot: &StatsSnapshot) {
        let capacity = self.capacity;
        if let Some(cpu_total) = snapshot.cpu_total() {
            Self::push_value(
                &mut self.cpu_total,
                capacity,
                cpu_total.round().clamp(0.0, 100.0) as u64,
            );
        }
        if let Some(used) = snapshot.ram_used_mb {
            Self::push_value(&mut self.ram_used, capacity, used);
        }
        if let Some(gpu_util) = snapshot.gpu_util {
            Self::push_value(
                &mut self.gpu_util,
                capacity,
                gpu_util.round().clamp(0.0, 100.0) as u64,
            );
        }
        if let Some(power_total) = snapshot.total_power_mw() {
            Self::push_value(&mut self.power_total, capacity, power_total);
        }
    }

    fn push_value(deque: &mut VecDeque<u64>, capacity: usize, value: u64) {
        if deque.len() >= capacity {
            deque.pop_front();
        }
        deque.push_back(value);
    }
}

#[derive(Debug)]
pub struct AppState {
    pub latest: Option<StatsSnapshot>,
    pub history: History,
    pub interval_ms: u64,
    pub show_help: bool,
    pub error: Option<String>,
    pub buttons: UiButtons,
    pub hover: HoverTarget,
}

impl AppState {
    pub fn new(interval_ms: u64, history_capacity: usize) -> Self {
        Self {
            latest: None,
            history: History::new(history_capacity),
            interval_ms,
            show_help: false,
            error: None,
            buttons: UiButtons::default(),
            hover: HoverTarget::None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct UiButton {
    pub x: u16,
    pub y: u16,
    pub width: u16,
}

impl UiButton {
    pub fn contains(&self, column: u16, row: u16) -> bool {
        row == self.y && column >= self.x && column < self.x.saturating_add(self.width)
    }
}

#[derive(Clone, Debug, Default)]
pub struct UiButtons {
    pub minus: Option<UiButton>,
    pub plus: Option<UiButton>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HoverTarget {
    #[default]
    None,
    Minus,
    Plus,
}
