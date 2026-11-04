// Датчики: ЦП, память, температура, сеть, диск, батарея — раз в секунду прямо из /proc и /sys
// (никаких процессов; только видеокарта NVIDIA — через nvidia-smi, у AMD всё есть в /sys). История по 120 точек — для графиков бара, всплывашек и виджетов.

use std::collections::VecDeque;
use std::fs;

pub const HIST: usize = 120;

#[derive(Default, Clone)]
pub struct Series {
    pub v: VecDeque<f32>,
}

impl Series {
    pub fn push(&mut self, x: f32) {
        if self.v.len() >= HIST {
            self.v.pop_front();
        }
        self.v.push_back(x);
    }
    pub fn max(&self) -> f32 {
        self.v.iter().copied().fold(0.0, f32::max)
    }
    /// последние n точек (старые — слева); недостающие — нули
    pub fn tail(&self, n: usize) -> Vec<f32> {
        let mut out = vec![0.0; n.saturating_sub(self.v.len())];
        let skip = self.v.len().saturating_sub(n);
        out.extend(self.v.iter().skip(skip));
        out
    }
}

#[derive(Default)]
pub struct Sys {
    pub cpu: f32,
    /// когда был последний замер (для бегущих графиков)
    pub at: Option<std::time::Instant>,
    pub mem: f32,
    pub mem_used: f64,
    pub mem_total: f64,
    pub swap: f32,
    pub temp: f32,
    pub gpu_temp: f32,
    pub gpu_load: f32,
    pub rx: f64,
    pub tx: f64,
    pub disk: f32,
    pub iface: String,
    pub ip: String,
    pub h_cpu: Series,
    pub h_mem: Series,
    pub h_temp: Series,
    pub h_gpu: Series,
    pub h_rx: Series,
    pub h_tx: Series,
    pub online: bool,
    prev_cpu: Option<(u64, u64)>,
    prev_net: Option<(u64, u64, std::time::Instant)>,
    temp_path: Option<String>,
    tick: u64,
    pub want_gpu: bool,
    gpu_rx: Option<std::sync::mpsc::Receiver<(f32, f32)>>,
    /// AMD (amdgpu): температура и загрузка из /sys, без процессов
    gpu_sys: Option<(String, Option<String>)>,
    /// power/runtime_status карты NVIDIA: на ноутбуке она спит (RTD3), а nvidia-smi её будит
    nv_pm: Option<String>,
    /// дискретная NVIDIA спит: датчики не спрашиваем
    pub gpu_sleep: bool,
    /// батарея ноутбука: заряд 0..100 и идёт ли зарядка; None — батареи нет
    pub bat: Option<(u8, bool)>,
}

/// датчики amdgpu: hwmon temp1_input (edge) и gpu_busy_percent той же карты
fn find_gpu() -> Option<(String, Option<String>)> {
    for e in fs::read_dir("/sys/class/hwmon").ok()?.flatten() {
        let p = e.path();
        if fs::read_to_string(p.join("name")).unwrap_or_default().trim() != "amdgpu" || !p.join("temp1_input").exists() {
            continue;
        }
        let busy = p.join("device/gpu_busy_percent");
        return Some((p.join("temp1_input").to_string_lossy().into(), busy.exists().then(|| busy.to_string_lossy().into())));
    }
    None
}

/// заряд батарей системы (не мышек и наушников: у них scope=Device); несколько — по энергии
fn read_bat() -> Option<(u8, bool)> {
    let (mut now, mut full, mut charging, mut any) = (0.0f64, 0.0f64, false, false);
    for e in fs::read_dir("/sys/class/power_supply").ok()?.flatten() {
        let p = e.path();
        let rd = |k: &str| fs::read_to_string(p.join(k)).map(|s| s.trim().to_string()).unwrap_or_default();
        if rd("type") != "Battery" || rd("scope") == "Device" || rd("present") == "0" {
            continue;
        }
        let num = |k: &str| rd(k).parse::<f64>().ok();
        let (n, f) = match (num("energy_now").or(num("charge_now")), num("energy_full").or(num("charge_full"))) {
            (Some(n), Some(f)) if f > 0.0 => (n, f),
            _ => match num("capacity") {
                Some(c) => (c, 100.0),
                None => continue,
            },
        };
        now += n;
        full += f;
        any = true;
        charging |= matches!(rd("status").as_str(), "Charging" | "Full");
    }
    any.then(|| (((now / full.max(1e-9)) * 100.0).round().clamp(0.0, 100.0) as u8, charging))
}

fn find_temp() -> Option<String> {
    let mut best: Option<(i32, String)> = None;
    for e in fs::read_dir("/sys/class/hwmon").ok()?.flatten() {
        let p = e.path();
        let name = fs::read_to_string(p.join("name")).unwrap_or_default().trim().to_string();
        let rank = match name.as_str() {
            "k10temp" | "zenpower" => 3,
            "coretemp" => 3,
            "cpu_thermal" | "acpitz" => 1,
            _ => 0,
        };
        if rank == 0 {
            continue;
        }
        let input = p.join("temp1_input");
        if input.exists() && best.as_ref().map(|b| b.0 < rank).unwrap_or(true) {
            best = Some((rank, input.to_string_lossy().to_string()));
        }
    }
    best.map(|b| b.1).or_else(|| {
        let p = "/sys/class/thermal/thermal_zone0/temp";
        std::path::Path::new(p).exists().then(|| p.to_string())
    })
}

/// карта NVIDIA: файл её состояния питания (только у PCI-устройства, не у разъёмов cardN-…)
fn find_nv_pm() -> Option<String> {
    for e in fs::read_dir("/sys/class/drm").ok()?.flatten() {
        let n = e.file_name().to_string_lossy().to_string();
        if !n.starts_with("card") || n.contains('-') {
            continue;
        }
        let d = e.path().join("device");
        if fs::read_to_string(d.join("vendor")).unwrap_or_default().trim() == "0x10de" {
            let p = d.join("power/runtime_status");
            return p.exists().then(|| p.to_string_lossy().to_string());
        }
    }
    None
}

impl Sys {
    pub fn new() -> Sys {
        let mut s = Sys { temp_path: find_temp(), gpu_sys: find_gpu(), nv_pm: find_nv_pm(), bat: read_bat(), online: true, ..Default::default() };
        s.sample();
        s
    }

    fn read_cpu() -> Option<(u64, u64)> {
        let t = fs::read_to_string("/proc/stat").ok()?;
        let line = t.lines().next()?;
        let f: Vec<u64> = line.split_whitespace().skip(1).filter_map(|x| x.parse().ok()).collect();
        let idle = f.get(3)? + f.get(4).unwrap_or(&0);
        Some((idle, f.iter().sum()))
    }

    fn read_net(&mut self) -> Option<(u64, u64)> {
        let route = fs::read_to_string("/proc/net/route").unwrap_or_default();
        // интерфейс маршрута по умолчанию
        let iface = route
            .lines()
            .skip(1)
            .find(|l| l.split_whitespace().nth(1) == Some("00000000"))
            .and_then(|l| l.split_whitespace().next())
            .map(String::from);
        self.online = iface.is_some();
        let t = fs::read_to_string("/proc/net/dev").ok()?;
        let (mut rx, mut tx) = (0u64, 0u64);
        for l in t.lines().skip(2) {
            let (name, rest) = l.split_once(':')?;
            let name = name.trim();
            let use_it = match &iface {
                Some(i) => name == i,
                None => name != "lo",
            };
            if !use_it {
                continue;
            }
            let f: Vec<u64> = rest.split_whitespace().filter_map(|x| x.parse().ok()).collect();
            rx += f.first().copied().unwrap_or(0);
            tx += f.get(8).copied().unwrap_or(0);
        }
        self.iface = iface.unwrap_or_default();
        Some((rx, tx))
    }

    pub fn sample(&mut self) {
        self.tick += 1;
        self.at = Some(std::time::Instant::now());
        if let Some((idle, total)) = Self::read_cpu() {
            if let Some((pi, pt)) = self.prev_cpu {
                let dt = total.saturating_sub(pt) as f32;
                if dt > 0.0 {
                    self.cpu = 1.0 - idle.saturating_sub(pi) as f32 / dt;
                }
            }
            self.prev_cpu = Some((idle, total));
        }
        if let Ok(t) = fs::read_to_string("/proc/meminfo") {
            let get = |k: &str| -> f64 {
                t.lines().find(|l| l.starts_with(k)).and_then(|l| l.split_whitespace().nth(1)).and_then(|v| v.parse().ok()).unwrap_or(0.0)
            };
            let total = get("MemTotal:");
            let avail = get("MemAvailable:");
            if total > 0.0 {
                self.mem = (1.0 - avail / total) as f32;
                self.mem_used = (total - avail) / 1048576.0;
                self.mem_total = total / 1048576.0;
            }
            let st = get("SwapTotal:");
            if st > 0.0 {
                self.swap = (1.0 - get("SwapFree:") / st) as f32;
            }
        }
        if let Some(p) = &self.temp_path {
            if let Ok(v) = fs::read_to_string(p) {
                self.temp = v.trim().parse::<f32>().unwrap_or(0.0) / 1000.0;
            }
        }
        if let Some((rx, tx)) = self.read_net() {
            let now = std::time::Instant::now();
            if let Some((prx, ptx, pt)) = self.prev_net {
                let dt = now.duration_since(pt).as_secs_f64().max(0.1);
                self.rx = rx.saturating_sub(prx) as f64 / dt;
                self.tx = tx.saturating_sub(ptx) as f64 / dt;
            }
            self.prev_net = Some((rx, tx, now));
        }
        // видеокарта: nvidia-smi — процесс, поэтому раз в 3 с и только если нужно
        if let Some(rx) = &self.gpu_rx {
            if let Ok((t, l)) = rx.try_recv() {
                self.gpu_temp = t;
                self.gpu_load = l;
            }
        }
        if let (true, Some((tp, busy))) = (self.want_gpu, &self.gpu_sys) {
            self.gpu_temp = fs::read_to_string(tp).ok().and_then(|v| v.trim().parse::<f32>().ok()).unwrap_or(0.0) / 1000.0;
            self.gpu_load = busy.as_ref().and_then(|b| fs::read_to_string(b).ok()).and_then(|v| v.trim().parse::<f32>().ok()).unwrap_or(0.0) / 100.0;
        } else if self.want_gpu && self.tick % 3 == 0 {
            // спящую дискретку (ноутбук, RTD3) не будим: nvidia-smi раз в 3 с не дал бы ей уснуть
            self.gpu_sleep = self.nv_pm.as_ref().and_then(|p| fs::read_to_string(p).ok()).map(|v| v.trim() == "suspended").unwrap_or(false);
        }
        if self.gpu_sleep {
            self.gpu_temp = 0.0;
            self.gpu_load = 0.0;
        } else if self.want_gpu && self.gpu_sys.is_none() && self.tick % 3 == 0 {
            let (tx, rx) = std::sync::mpsc::channel();
            self.gpu_rx = Some(rx);
            std::thread::spawn(move || {
                if let Ok(o) = std::process::Command::new("nvidia-smi").args(["--query-gpu=temperature.gpu,utilization.gpu", "--format=csv,noheader,nounits"]).output() {
                    let s = String::from_utf8_lossy(&o.stdout);
                    let mut it = s.trim().split(',').map(|x| x.trim().parse::<f32>().unwrap_or(0.0));
                    let _ = tx.send((it.next().unwrap_or(0.0), it.next().unwrap_or(0.0) / 100.0));
                }
            });
        }
        // батарея меняется медленно: раз в 5 с (и сразу, если её вставили/вынули — тоже за 5 с)
        if self.tick % 5 == 0 {
            self.bat = read_bat();
        }
        if self.tick % 30 == 1 {
            if let Ok(st) = nix_statvfs("/") {
                self.disk = st;
            }
            self.ip = local_ip(&self.iface);
        }
        self.h_cpu.push(self.cpu);
        self.h_mem.push(self.mem);
        self.h_temp.push(self.temp);
        self.h_gpu.push(self.gpu_temp);
        self.h_rx.push(self.rx as f32);
        self.h_tx.push(self.tx as f32);
    }

    pub fn series(&self, src: &str) -> (&Series, f32) {
        // источник и «полная шкала» для графика
        match src {
            "ram" => (&self.h_mem, 1.0),
            "temp" => (&self.h_temp, 100.0),
            "gpu" => (&self.h_gpu, 100.0),
            "rx" => (&self.h_rx, self.h_rx.max().max(64.0 * 1024.0)),
            "tx" => (&self.h_tx, self.h_tx.max().max(64.0 * 1024.0)),
            _ => (&self.h_cpu, 1.0),
        }
    }
}

fn nix_statvfs(path: &str) -> Result<f32, ()> {
    let c = std::ffi::CString::new(path).map_err(|_| ())?;
    let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut st) } != 0 {
        return Err(());
    }
    let total = st.f_blocks as f64 * st.f_frsize as f64;
    let free = st.f_bavail as f64 * st.f_frsize as f64;
    Ok(if total > 0.0 { (1.0 - free / total) as f32 } else { 0.0 })
}

fn local_ip(iface: &str) -> String {
    if iface.is_empty() {
        return String::new();
    }
    let o = std::process::Command::new("ip").args(["-4", "-o", "addr", "show", iface]).output();
    o.ok()
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout).to_string();
            s.split_whitespace().skip_while(|w| *w != "inet").nth(1).map(|x| x.split('/').next().unwrap_or("").to_string())
        })
        .unwrap_or_default()
}

/// 1536 → «1.5K», в байтах в секунду
pub fn rate(b: f64) -> String {
    if b < 1024.0 {
        format!("{:.0}B", b)
    } else if b < 1024.0 * 1024.0 {
        format!("{:.0}K", b / 1024.0)
    } else if b < 1024.0 * 1024.0 * 1024.0 {
        format!("{:.1}M", b / 1048576.0)
    } else {
        format!("{:.1}G", b / 1073741824.0)
    }
}

/// своя память, МБ (для «о шелле»)
pub fn self_rss_mb() -> f32 {
    fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|t| t.lines().find(|l| l.starts_with("VmRSS:")).and_then(|l| l.split_whitespace().nth(1)).and_then(|v| v.parse::<f32>().ok()))
        .map(|kb| kb / 1024.0)
        .unwrap_or(0.0)
}
