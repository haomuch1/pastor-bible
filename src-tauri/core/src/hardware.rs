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
    /// Which drive `free_disk_gb` was measured on, e.g. "C:". P7's laptop was
    /// told 80 GB by this screen and 35.8 GB by the installer, and had no way
    /// to know the two were talking about different things.
    pub disk_drive: String,
    /// Every graphics device the model server can see, with its memory. The
    /// same probe Settings > Compute uses; this screen used to run its own,
    /// which reported one adapter from the OS and said the card was not used.
    pub gpu_devices: Vec<crate::compute::GpuDevice>,
    /// What the graphics devices mean for this reader, in plain words. Not a
    /// warning and never part of `below`: a card too small for the standard
    /// model is an ordinary machine, and the processor answers either way.
    pub graphics: String,
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
    #[cfg(target_os = "macos")]
    {
        // hw.memsize is the machine's physical memory in bytes, and on Apple
        // Silicon it is also the memory the graphics side is drawing on.
        if let Some(v) = crate::sidecar::macos::sysctl("hw.memsize") {
            if let Ok(bytes) = v.parse::<f64>() {
                return bytes / (1024.0 * 1024.0 * 1024.0);
            }
        }
        return 0.0;
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
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
    #[cfg(target_os = "macos")]
    {
        // statvfs, which POSIX has had for decades and `libc` exports for every
        // unix. f_bavail is what a non-privileged process may actually have,
        // which is the figure the reader cares about.
        use std::ffi::CString;
        let Ok(c) = CString::new(path) else { return 0.0 };
        unsafe {
            let mut st: libc::statvfs = std::mem::zeroed();
            if libc::statvfs(c.as_ptr(), &mut st) == 0 {
                let unit = if st.f_frsize > 0 { st.f_frsize } else { st.f_bsize };
                return st.f_bavail as f64 * unit as f64 / (1024.0 * 1024.0 * 1024.0);
            }
        }
        0.0
    }
    // Linux still answers 0.0, and `probe` therefore still shows no disk line
    // there. One `#[cfg(unix)]` would have covered both, and would have been a
    // behaviour change on a platform this project has never run a package on.
    // DECISIONS records the choice; it is flagged rather than done quietly.
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        let _ = path;
        0.0
    }
}

/// The drive a path is on, as a person writes it: "C:" on Windows, "/" on
/// anything else. Shown beside a free-space figure so it cannot be mistaken
/// for a different drive's.
pub fn drive_of(path: &str) -> String {
    #[cfg(windows)]
    {
        let p = path.trim();
        let b = p.as_bytes();
        if b.len() >= 2 && b[1] == b':' {
            return format!("{}:", (b[0] as char).to_ascii_uppercase());
        }
        String::new()
    }
    #[cfg(target_os = "macos")]
    {
        // "There is about 12.4 GB free on /" tells a Mac reader nothing they
        // can act on. Everything the app writes is under their home folder, so
        // the volume that matters is the one the Mac boots from.
        let _ = path;
        "this Mac's disk".to_string()
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        let _ = path;
        "/".to_string()
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
    #[cfg(target_os = "macos")]
    {
        // "Apple M2 Pro" on Apple Silicon; "Intel(R) Core(TM) i7-9750H CPU @
        // 2.60GHz" on an Intel Mac. Either way it is the string the reader
        // would see in About This Mac.
        return crate::sidecar::macos::sysctl("machdep.cpu.brand_string")
            .unwrap_or_else(|| "unknown".to_string());
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
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
    } else if cfg!(target_os = "macos") {
        // Not `std::env::consts::OS`, which says "macos". Apple writes it
        // "macOS" and so does every Mac this will run on.
        if cfg!(target_arch = "x86_64") {
            "macOS (Intel)".to_string()
        } else {
            "macOS (Apple Silicon)".to_string()
        }
    } else {
        std::env::consts::OS.to_string()
    }
}

/// The one line an Intel Mac is told about its own speed, or None everywhere
/// else.
///
/// This is not a probe result and must not be: llama.cpp's x64 macOS build
/// contains no Metal backend at all and its `llama-server` references none, so
/// on any Intel Mac, with any card, this program answers on the processor. It
/// is a fact about the build, which is why it is a build-time `cfg` rather than
/// something read off the machine — the graphics sentence P7-fix-2 had to
/// rewrite was wrong precisely because it drew a conclusion from a probe that
/// could not support it.
pub fn no_gpu_platform_note() -> Option<String> {
    if cfg!(target_os = "macos") && cfg!(target_arch = "x86_64") {
        Some(
            "This Mac has no graphics card The Pastor Bible can use; answers will take \
             several minutes. The smaller model is faster."
                .to_string(),
        )
    } else {
        None
    }
}

/// What the graphics devices mean, said the way Settings > Compute says it.
///
/// P7's laptop was told "No separate graphics card was found. The Pastor Bible
/// does not use one yet." It has an RTX 3050, and the app has used graphics
/// cards since P6. Both halves were wrong.
pub fn graphics_sentence(devices: &[crate::compute::GpuDevice], needs_mib: u64) -> String {
    if devices.is_empty() {
        return "No graphics card was found that The Pastor Bible can use, so the \
                processor will answer. That works; it takes minutes rather than \
                seconds."
            .to_string();
    }
    let mut parts = Vec::new();

    // Apple Silicon: the "card" is the machine's own memory, shared with
    // everything else running on it. Printing a total the way a discrete card's
    // total is printed would tell the reader they have room they may not have,
    // so the free figure is what is shown and what the rule is applied to. The
    // rule itself is unchanged: the model's measured requirement plus a tenth,
    // against free memory, which is what it has always compared.
    if cfg!(target_os = "macos") {
        for d in devices {
            let free_gb = d.free_mib as f64 / 1024.0;
            let needs_gb = needs_mib as f64 / 1024.0;
            if d.free_mib >= needs_mib {
                parts.push(format!(
                    "{}, shared memory: {:.1} GB free right now, so the standard model \
                     will run on it.",
                    d.name, free_gb
                ));
            } else {
                parts.push(format!(
                    "{}, shared memory: {:.1} GB free right now and the standard model \
                     needs {:.1} GB, so the processor will answer instead. The smaller \
                     model in Settings needs less.",
                    d.name, free_gb, needs_gb
                ));
            }
        }
        return parts.join(" ");
    }

    for d in devices {
        let gb = d.total_mib as f64 / 1024.0;
        if d.free_mib >= needs_mib {
            parts.push(format!(
                "{}, {:.1} GB: big enough for the standard model, which will run on it.",
                d.name, gb
            ));
        } else {
            parts.push(format!(
                "{}, {:.1} GB: too small for the standard model, which will run on the \
                 processor instead. The smaller model in Settings can use it.",
                d.name, gb
            ));
        }
    }
    parts.join(" ")
}

/// Read the machine, and say in one sentence what is below the reference.
pub fn probe(disk_path: &str, llama_server: &str, needs_mib: u64) -> Hardware {
    let ram = total_ram_gb();
    let free_disk = free_disk_gb(disk_path);
    let drive = drive_of(disk_path);
    let gpu = gpu_name();
    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(0);

    // The same probe Settings > Compute uses. This screen used to call
    // gpu_name(), which asks the OS for one display adapter and knows nothing
    // about whether a model fits on it. On P7's laptop that produced "No
    // separate graphics card was found" on a machine with an RTX 3050.
    let devices = crate::compute::list_devices(llama_server).unwrap_or_default();

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
            "There is about {:.1} GB free on {}, where The Pastor Bible keeps its \
             files; it and its answering model need about {:.0} GB.",
            free_disk,
            if drive.is_empty() { "this drive".to_string() } else { drive.clone() },
            REFERENCE.disk_gb
        ));
    }
    // Deliberately not pushed into `below`: see the field's own note.

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
        disk_drive: drive,
        graphics: graphics_sentence(&devices, needs_mib),
        gpu_devices: devices,
        os: os_name(),
        reference: REFERENCE,
        below,
        warning,
    }
}
