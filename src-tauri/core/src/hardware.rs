//! What this machine has, beside what the reference machine has.
//!
//! PLAN 6.5 as amended on 2026-08-26: The Pastor Bible is built and tested on
//! one machine, named here. Below it the app still runs, slower. Nothing in
//! this file refuses anything; it exists so the first-run screen can show the
//! reader an honest comparison and one plain sentence, and then let them
//! continue.

use serde::Serialize;

/// The machine every measured number in EVAL.md and README came from.
pub const REFERENCE: Reference = Reference {
    cpu: "AMD Ryzen 7 5800X (8 cores)",
    gpu: "NVIDIA RTX 3080 10 GB",
    ram_gb: 32.0,
    os: "Windows 11",
    disk_gb: 6.0,
};

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Reference {
    pub cpu: &'static str,
    pub gpu: &'static str,
    pub ram_gb: f64,
    pub os: &'static str,
    /// Free disk the install and the model download need between them.
    pub disk_gb: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Hardware {
    pub cpu: String,
    pub cores: usize,
    pub gpu: String,
    pub ram_gb: f64,
    pub free_ram_gb: f64,
    pub free_disk_gb: f64,
    pub os: String,
    pub reference: Reference,
    /// One line per thing that is below the reference machine. Empty when
    /// nothing is.
    pub below: Vec<String>,
    /// The single sentence the first-run screen shows, if any.
    pub warning: Option<String>,
}

pub fn total_ram_gb() -> f64 {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
        unsafe {
            let mut st: MEMORYSTATUSEX = std::mem::zeroed();
            st.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
            if GlobalMemoryStatusEx(&mut st) != 0 {
                return st.ullTotalPhys as f64 / (1024.0 * 1024.0 * 1024.0);
            }
        }
        0.0
    }
    #[cfg(not(windows))]
    {
        if let Ok(text) = std::fs::read_to_string("/proc/meminfo") {
            for line in text.lines() {
                if let Some(rest) = line.strip_prefix("MemTotal:") {
                    if let Some(kb) = rest.split_whitespace().next() {
                        if let Ok(kb) = kb.parse::<f64>() {
                            return kb / (1024.0 * 1024.0);
                        }
                    }
                }
            }
        }
        0.0
    }
}

/// Free space on the volume holding `path`, in GB.
pub fn free_disk_gb(path: &str) -> f64 {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
        let mut wide: Vec<u16> = path.encode_utf16().collect();
        wide.push(0);
        unsafe {
            let mut free: u64 = 0;
            let ok = GetDiskFreeSpaceExW(
                wide.as_ptr(),
                &mut free,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            if ok != 0 {
                return free as f64 / (1024.0 * 1024.0 * 1024.0);
            }
        }
        0.0
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        0.0
    }
}

/// The display adapter's own description, from the OS.
///
/// EnumDisplayDevices rather than WMI: it answers in microseconds and needs no
/// subprocess, and this runs while a reader is waiting on the first screen.
pub fn gpu_name() -> String {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Graphics::Gdi::{EnumDisplayDevicesW, DISPLAY_DEVICEW};
        unsafe {
            let mut best = String::new();
            for i in 0..8u32 {
                let mut dd: DISPLAY_DEVICEW = std::mem::zeroed();
                dd.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;
                if EnumDisplayDevicesW(std::ptr::null(), i, &mut dd, 0) == 0 {
                    break;
                }
                let name = String::from_utf16_lossy(&dd.DeviceString)
                    .trim_end_matches('\u{0}')
                    .trim()
                    .to_string();
                if name.is_empty() {
                    continue;
                }
                // Prefer a real adapter over a Remote Desktop or mirror device.
                if best.is_empty() || (best.contains("Remote") && !name.contains("Remote")) {
                    best = name;
                }
            }
            if best.is_empty() {
                "unknown".to_string()
            } else {
                best
            }
        }
    }
    #[cfg(not(windows))]
    {
        "unknown".to_string()
    }
}

pub fn cpu_name() -> String {
    #[cfg(windows)]
    {
        // The processor's own name string, as the OS recorded it at boot.
        let key = r"HARDWARE\DESCRIPTION\System\CentralProcessor\0";
        registry_string(key, "ProcessorNameString").unwrap_or_else(|| "unknown".to_string())
    }
    #[cfg(not(windows))]
    {
        if let Ok(text) = std::fs::read_to_string("/proc/cpuinfo") {
            for line in text.lines() {
                if let Some(rest) = line.split_once(':').filter(|(k, _)| k.trim() == "model name") {
                    return rest.1.trim().to_string();
                }
            }
        }
        "unknown".to_string()
    }
}

#[cfg(windows)]
fn registry_string(key: &str, value: &str) -> Option<String> {
    use windows_sys::Win32::System::Registry::{
        RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ,
    };
    let mut k: Vec<u16> = key.encode_utf16().collect();
    k.push(0);
    let mut v: Vec<u16> = value.encode_utf16().collect();
    v.push(0);
    unsafe {
        let mut buf = [0u16; 256];
        let mut len = (buf.len() * 2) as u32;
        let rc = RegGetValueW(
            HKEY_LOCAL_MACHINE,
            k.as_ptr(),
            v.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            buf.as_mut_ptr() as *mut std::ffi::c_void,
            &mut len,
        );
        if rc != 0 {
            return None;
        }
        let n = (len as usize / 2).saturating_sub(1).min(buf.len());
        Some(String::from_utf16_lossy(&buf[..n]).trim().to_string())
    }
}

pub fn os_name() -> String {
    if cfg!(windows) {
        "Windows".to_string()
    } else if cfg!(target_os = "linux") {
        "Linux".to_string()
    } else {
        std::env::consts::OS.to_string()
    }
}

/// Read the machine, and say in one sentence what is below the reference.
pub fn probe(disk_path: &str) -> Hardware {
    let ram = total_ram_gb();
    let free_disk = free_disk_gb(disk_path);
    let gpu = gpu_name();
    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(0);

    let mut below = Vec::new();
    if ram > 0.0 && ram + 0.5 < REFERENCE.ram_gb {
        below.push(format!(
            "This computer has about {:.0} GB of memory; the machine The Pastor Bible \
             is tested on has {:.0} GB.",
            ram, REFERENCE.ram_gb
        ));
    }
    if free_disk > 0.0 && free_disk < REFERENCE.disk_gb {
        below.push(format!(
            "There is about {:.1} GB free on this drive; The Pastor Bible and its \
             answering model need about {:.0} GB.",
            free_disk, REFERENCE.disk_gb
        ));
    }
    // The GPU is not used yet, so it is reported and never warned about beyond
    // one line: the answer time in README is a processor time either way.
    let has_discrete = {
        let g = gpu.to_lowercase();
        g.contains("nvidia") || g.contains("radeon") || g.contains("geforce") || g.contains("arc")
    };
    if !has_discrete {
        below.push(
            "No separate graphics card was found. The Pastor Bible does not use one yet, \
             so this changes nothing today."
                .to_string(),
        );
    }

    let warning = if below.is_empty() {
        None
    } else {
        Some(
            "This computer is below the machine The Pastor Bible was tested on. It will \
             still work; answers may take longer. You can carry on."
                .to_string(),
        )
    };

    Hardware {
        cpu: cpu_name(),
        cores,
        gpu,
        ram_gb: ram,
        free_ram_gb: crate::sidecar::free_ram_gb(),
        free_disk_gb: free_disk,
        os: os_name(),
        reference: REFERENCE,
        below,
        warning,
    }
}
