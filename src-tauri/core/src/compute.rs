//! Which processor answers the question.
//!
//! P4 measured the same question at 12 seconds with the model on the graphics
//! card and 178 seconds on the processor. That is the difference between an
//! answer a reader waits for and an answer they walk away from, so the app uses
//! the card when the card can hold the model, and says which it used.
//!
//! ## There is one server, not two
//!
//! Measured on 2026-08-27 on both Windows and Linux: every file in llama.cpp's
//! Vulkan release archive is byte-identical to the file of the same name in the
//! CPU archive, and the Vulkan archive holds exactly one extra file —
//! `ggml-vulkan.dll`, or `libggml-vulkan.so`. ggml loads its backends as
//! dynamic libraries at run time, so that one file beside the CPU build is the
//! whole difference. The installer therefore ships one `llama-server` with the
//! Vulkan backend among its libraries, and `-ngl` decides at launch which
//! processor runs the model.
//!
//! ## How a device is found
//!
//! `llama-server --list-devices` prints what the backends can see and exits.
//! It loads no model, takes about a second, and needs no GPU-specific code in
//! this program:
//!
//!     Available devices:
//!       Vulkan0: NVIDIA GeForce RTX 3080 (10267 MiB, 9495 MiB free)
//!
//! and, from the same binary on a machine with no usable device:
//!
//!     Available devices:
//!       (none)
//!
//! The free figure is what the decision rests on, not the total: a card with
//! ten gigabytes and nine already spoken for cannot hold an eight-billion
//! parameter model, and starting anyway would fail slowly rather than quickly.
//!
//! ## What a Mac reports
//!
//! Measured on 2026-09-01, on GitHub's own runners, from the shipped binaries.
//! Apple Silicon:
//!
//!     Available devices:
//!       MTL0: Apple Paravirtual device (4778 MiB, 4778 MiB free)
//!       BLAS: Accelerate (0 MiB, 0 MiB free)
//!
//! and an Intel Mac, which has no Metal backend in its build at all:
//!
//!     Available devices:
//!       BLAS: Accelerate (0 MiB, 0 MiB free)
//!
//! Two things follow. The Metal backend calls itself **MTL**, not Metal, and
//! the device name there is a virtual machine's -- a real Mac says "Apple M2
//! Pro" or the like, and nobody here has seen one.
//!
//! ## Not every line is a graphics card
//!
//! The macOS builds register the Accelerate framework as a ggml device, and it
//! prints beside the real ones, and on an Intel Mac it prints alone where
//! "(none)" would be on Windows.
//!
//! Accelerate is the processor doing matrix arithmetic faster; it is not
//! somewhere a model can live. Shown to a reader it becomes a graphics card
//! with 0.0 GB, which is worse than saying nothing, and it is exactly the kind
//! of sentence P7-fix-2 had to rewrite. A device reporting no memory at all is
//! therefore dropped here, at the parse, so that no screen and no decision ever
//! sees it. The rule is principled rather than a special case for one name:
//! the choice rests on whether a device can hold the model, and a device with
//! no memory can hold nothing.

use std::process::Command;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Every layer. llama.cpp clamps this to the model's own layer count, so any
/// number past the largest model works and none has to be looked up.
pub const FULL_OFFLOAD: u32 = 99;

/// A device `--list-devices` reported.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct GpuDevice {
    /// The backend's own label, "Vulkan0".
    pub id: String,
    /// What a reader would recognise: "NVIDIA GeForce RTX 3080".
    pub name: String,
    pub total_mib: u64,
    pub free_mib: u64,
}

/// What the app decided, and why, in words a reader can be shown.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComputeChoice {
    /// What Settings asks for: "auto", "cpu" or "gpu".
    pub mode: String,
    /// What will actually run: "cpu" or "gpu".
    pub using: String,
    /// The device found, if any was.
    pub device: Option<GpuDevice>,
    /// The free memory the chosen model needs, for the message.
    pub needs_mib: u64,
    /// One sentence, shown in Settings.
    pub reason: String,
}

impl ComputeChoice {
    pub fn gpu_layers(&self) -> u32 {
        if self.using == "gpu" {
            FULL_OFFLOAD
        } else {
            0
        }
    }
}

/// Ask the server what it can see.
///
/// Returns an empty list when there is no device, and an error only when the
/// server could not be run at all. Both end at the processor; they are kept
/// apart so the reason shown to the reader is the true one.
pub fn list_devices(server: &str) -> Result<Vec<GpuDevice>, String> {
    // A test hook, and nothing else: the fallback path is the one that must
    // work on the machines we do not have, so it has to be reachable on the
    // machine we do.
    if std::env::var("TPB_NO_GPU").is_ok() {
        return Ok(Vec::new());
    }

    let out = Command::new(server)
        .arg("--list-devices")
        .output()
        .map_err(|e| format!("cannot run {}: {}", server, e))?;

    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(parse_devices(&text))
}

/// Parse the device list. Written against the real output, pasted above.
pub fn parse_devices(text: &str) -> Vec<GpuDevice> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line == "(none)" || line.ends_with("devices:") {
            continue;
        }
        // "Vulkan0: NVIDIA GeForce RTX 3080 (10267 MiB, 9495 MiB free)"
        let Some((id, rest)) = line.split_once(':') else { continue };
        if id.is_empty() || id.contains(' ') {
            continue;
        }
        let Some((name, tail)) = rest.rsplit_once('(') else { continue };
        let tail = tail.trim_end_matches(')');
        let mut nums = tail.split(',').map(|p| mib(p));
        let (Some(Some(total)), Some(Some(free))) = (nums.next(), nums.next()) else { continue };
        // "BLAS: Accelerate (0 MiB, 0 MiB free)" and anything else that reports
        // no memory of its own. See the note at the top of this file.
        if total == 0 {
            continue;
        }
        out.push(GpuDevice {
            id: id.trim().to_string(),
            name: name.trim().to_string(),
            total_mib: total,
            free_mib: free,
        });
    }
    out
}

/// "10267 MiB" or "9495 MiB free" -> 10267 / 9495.
fn mib(part: &str) -> Option<u64> {
    part.trim().split_whitespace().next()?.parse::<u64>().ok()
}

/// Decide, for one model, with one setting.
///
/// `needs_mib` is the measured figure for that model plus a tenth; see
/// `download::ModelSpec::vram_mib`.
pub fn decide(mode: &str, server: &str, needs_mib: u64) -> ComputeChoice {
    let choice = |using: &str, device: Option<GpuDevice>, reason: String| ComputeChoice {
        mode: mode.to_string(),
        using: using.to_string(),
        device,
        needs_mib,
        reason,
    };

    if mode == "cpu" {
        return choice("cpu", None, "Set to use the processor.".into());
    }

    let devices = match list_devices(server) {
        Ok(d) => d,
        Err(e) => {
            let why = format!("The graphics card could not be checked ({}). Using the processor.", e);
            return choice("cpu", None, why);
        }
    };

    let best = devices.into_iter().max_by_key(|d| d.free_mib);
    match best {
        None if mode == "gpu" => choice(
            "cpu",
            None,
            "No graphics card that can run a model was found, so the processor is being \
             used instead."
                .into(),
        ),
        None => choice("cpu", None, "No graphics card that can run a model was found.".into()),
        Some(d) => {
            if mode == "gpu" {
                // An explicit choice is honoured, and the risk is stated.
                let why = if d.free_mib >= needs_mib {
                    format!("Using {}.", d.name)
                } else {
                    format!(
                        "Using {}, which reports {} MB free against the {} MB this model \
                         needs. If it will not load, choose the processor.",
                        d.name, d.free_mib, needs_mib
                    )
                };
                return choice("gpu", Some(d), why);
            }
            if d.free_mib >= needs_mib {
                let why = format!("Using {}, which has room for this model.", d.name);
                choice("gpu", Some(d), why)
            } else {
                let why = format!(
                    "{} has {} MB free and this model needs {} MB, so the processor is being \
                     used. The smaller model in Settings fits more cards.",
                    d.name, d.free_mib, needs_mib
                );
                choice("cpu", Some(d), why)
            }
        }
    }
}

/// How long a reader should expect to wait, said before they commit to waiting.
pub fn answer_time_hint(using: &str) -> &'static str {
    if using == "gpu" {
        "Answers usually take under a minute on this machine."
    } else {
        "Answers take a few minutes on this machine."
    }
}

/// The probe is cheap but not free, and nothing about a machine's graphics card
/// changes between two questions. One second, once.
pub const PROBE_CACHE: Duration = Duration::from_secs(300);

#[cfg(test)]
mod tests {
    use super::*;

    const REAL: &str = "Available devices:\n  \
        Vulkan0: NVIDIA GeForce RTX 3080 (10267 MiB, 9495 MiB free)\n";
    const NONE: &str = "Available devices:\n  (none)\n";

    #[test]
    fn the_real_output_parses() {
        let d = parse_devices(REAL);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].id, "Vulkan0");
        assert_eq!(d[0].name, "NVIDIA GeForce RTX 3080");
        assert_eq!(d[0].total_mib, 10267);
        assert_eq!(d[0].free_mib, 9495);
    }

    #[test]
    fn a_machine_with_no_device_yields_none() {
        assert!(parse_devices(NONE).is_empty());
        assert!(parse_devices("").is_empty());
    }

    /// The whole of what an Intel Mac reports, pasted from a macOS 15 runner on
    /// 2026-09-01. Accelerate is the processor, not a card, and the reader must
    /// never be shown it as "0.0 GB free".
    #[test]
    fn accelerate_is_not_a_graphics_card() {
        let text = "Available devices:\n  BLAS: Accelerate (0 MiB, 0 MiB free)\n";
        assert!(
            parse_devices(text).is_empty(),
            "Accelerate was taken for a device: {:?}",
            parse_devices(text)
        );
    }

    /// And it must not crowd out a real one when both are listed. This is the
    /// whole of what an Apple Silicon Mac printed on 2026-09-01, pasted; the
    /// backend calls itself MTL and the name is a virtual machine's.
    #[test]
    fn a_real_device_beside_accelerate_survives_alone() {
        let text = "Available devices:\n  \
            MTL0: Apple Paravirtual device (4778 MiB, 4778 MiB free)\n  \
            BLAS: Accelerate (0 MiB, 0 MiB free)\n";
        let d = parse_devices(text);
        assert_eq!(d.len(), 1, "{:?}", d);
        assert_eq!(d[0].id, "MTL0");
        assert_eq!(d[0].name, "Apple Paravirtual device");
        assert_eq!(d[0].free_mib, 4778);
    }

    /// The Windows output this file was written against is unchanged by the
    /// rule above: nothing on Windows reports zero.
    #[test]
    fn the_windows_output_is_untouched_by_the_zero_memory_rule() {
        let d = parse_devices(REAL);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].total_mib, 10267);
    }

    #[test]
    fn two_devices_are_both_read_and_the_freest_wins() {
        let text = "Available devices:\n  \
            Vulkan0: NVIDIA GeForce RTX 3080 (10267 MiB, 2000 MiB free)\n  \
            Vulkan1: AMD Radeon RX 7900 XTX (24560 MiB, 24000 MiB free)\n";
        let d = parse_devices(text);
        assert_eq!(d.len(), 2);
        assert_eq!(d[1].name, "AMD Radeon RX 7900 XTX");
        assert_eq!(d.into_iter().max_by_key(|x| x.free_mib).unwrap().free_mib, 24000);
    }

    #[test]
    fn a_name_with_brackets_in_it_keeps_its_numbers() {
        let text = "Available devices:\n  \
            Vulkan0: Intel(R) Arc(TM) A770 Graphics (16384 MiB, 15000 MiB free)\n";
        let d = parse_devices(text);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].name, "Intel(R) Arc(TM) A770 Graphics");
        assert_eq!(d[0].free_mib, 15000);
    }
}
