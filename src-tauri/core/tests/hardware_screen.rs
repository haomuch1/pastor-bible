//! The "This computer" screen must be right about graphics and about which
//! drive it measured.
//!
//! P7's laptop -- an RTX 3050 machine -- was told "No separate graphics card
//! was found. The Pastor Bible does not use one yet." Both halves were untrue:
//! the card was there, and the app has used graphics cards since P6. The
//! screen was asking the OS for one display adapter instead of asking the
//! model server what it can actually run on, which is what Settings > Compute
//! had been doing all along.
//!
//! It also reported free disk without saying which drive, and the number
//! disagreed with the installer's.

use pastor_bible_core::compute::GpuDevice;
use pastor_bible_core::hardware;

fn dev(name: &str, total_mib: u64, free_mib: u64) -> GpuDevice {
    GpuDevice { id: "Vulkan0".to_string(), name: name.to_string(), total_mib, free_mib }
}

#[test]
fn no_device_says_the_processor_will_answer() {
    let s = hardware::graphics_sentence(&[], 6325);
    assert!(s.contains("processor"), "{}", s);
    assert!(
        !s.to_lowercase().contains("not use one yet"),
        "the old untruth is back: {}",
        s
    );
    assert!(!s.to_lowercase().contains("separate graphics card"), "{}", s);
}

#[test]
#[cfg(not(target_os = "macos"))]
fn a_card_too_small_says_so_and_names_the_smaller_model() {
    // P7's laptop: 4 GB card, standard model needs 6,325 MiB free.
    let s = hardware::graphics_sentence(&[dev("NVIDIA GeForce RTX 3050 Laptop GPU", 4096, 3400)], 6325);
    assert!(s.contains("RTX 3050"), "{}", s);
    assert!(s.contains("4.0 GB"), "{}", s);
    assert!(s.contains("too small"), "{}", s);
    assert!(s.contains("processor"), "{}", s);
    assert!(s.contains("smaller model"), "the reader is not told what would fit: {}", s);
}

#[test]
#[cfg(not(target_os = "macos"))]
fn a_card_big_enough_says_it_will_be_used() {
    // This machine: 10 GB card with room.
    let s = hardware::graphics_sentence(&[dev("NVIDIA GeForce RTX 3080", 10267, 9495)], 6325);
    assert!(s.contains("RTX 3080"), "{}", s);
    assert!(s.contains("big enough"), "{}", s);
    assert!(!s.contains("too small"), "{}", s);
}

/// Apple Silicon: the device is the machine's own memory, so the sentence says
/// so and reports what is free rather than a total that is shared with
/// everything else running.
#[test]
#[cfg(target_os = "macos")]
fn apple_silicon_says_shared_memory_and_reports_what_is_free() {
    // 16 GB machine with 9.8 GB of it free; the standard model needs 6,325 MiB.
    let s = hardware::graphics_sentence(&[dev("Apple M2 Pro", 16384, 10035)], 6325);
    assert!(s.contains("Apple M2 Pro"), "{}", s);
    assert!(s.contains("shared memory"), "{}", s);
    assert!(s.contains("9.8 GB free"), "the free figure is not the one shown: {}", s);
    assert!(s.contains("the standard model will run on it"), "{}", s);
    assert!(
        !s.contains("16.0 GB"),
        "the total was printed, which tells the reader they have room they may not have: {}",
        s
    );
}

/// The same machine with most of its memory already spoken for. The rule is
/// the measured requirement against the free figure, exactly as elsewhere.
#[test]
#[cfg(target_os = "macos")]
fn apple_silicon_short_of_memory_sends_it_to_the_processor() {
    let s = hardware::graphics_sentence(&[dev("Apple M1", 8192, 3072)], 6325);
    assert!(s.contains("Apple M1"), "{}", s);
    assert!(s.contains("shared memory"), "{}", s);
    assert!(s.contains("3.0 GB free"), "{}", s);
    assert!(s.contains("processor will answer"), "{}", s);
    assert!(s.contains("smaller model"), "{}", s);
}

#[test]
fn every_device_is_listed_not_just_the_first() {
    // The laptop has two: an RTX 3050 and Intel Iris Xe. The old screen showed
    // one adapter and drew a conclusion from it.
    let s = hardware::graphics_sentence(
        &[dev("NVIDIA GeForce RTX 3050 Laptop GPU", 4096, 3400), dev("Intel Iris Xe Graphics", 2048, 1800)],
        6325,
    );
    assert!(s.contains("RTX 3050"), "{}", s);
    assert!(s.contains("Iris Xe"), "{}", s);
}

#[test]
fn the_drive_is_named() {
    if cfg!(windows) {
        assert_eq!(hardware::drive_of(r"C:\Users\someone\AppData\Roaming\x"), "C:");
        assert_eq!(hardware::drive_of(r"d:\elsewhere"), "D:");
        assert_eq!(hardware::drive_of(r"\server\share\x"), "");
    } else if cfg!(target_os = "macos") {
        // "free on /" is not something a Mac reader can act on.
        assert_eq!(
            hardware::drive_of("/Users/someone/Library/Application Support/x"),
            "this Mac's disk"
        );
    } else {
        assert_eq!(hardware::drive_of("/home/someone"), "/");
    }
}

/// The one line an Intel Mac carries, and the silence everywhere else.
///
/// It is a build fact, not a probe: llama.cpp's x64 macOS build has no Metal
/// backend in it at all, so an Intel Mac answers on the processor whatever card
/// it has. The graphics sentence P7-fix-2 had to rewrite was wrong because it
/// drew a conclusion a probe could not support; this one draws its conclusion
/// from the files that shipped.
#[test]
fn only_an_intel_mac_is_warned_about_its_speed() {
    let note = hardware::no_gpu_platform_note();
    if cfg!(target_os = "macos") && cfg!(target_arch = "x86_64") {
        let n = note.expect("an Intel Mac build must carry the note");
        assert!(n.contains("no graphics card"), "{}", n);
        assert!(n.contains("several minutes"), "{}", n);
        assert!(n.contains("smaller model"), "the reader is not told what is faster: {}", n);
        assert!(!n.to_lowercase().contains("space"), "memory is never called space: {}", n);
    } else {
        assert!(note.is_none(), "a non-Intel-Mac build must say nothing: {:?}", note);
    }
}

/// macOS reports memory through `vm_stat` and `sysctl` rather than Mach, and
/// both must answer with something a person would recognise as this machine.
#[test]
#[cfg(target_os = "macos")]
fn the_mac_readings_are_real_numbers() {
    let total = hardware::total_ram_gb();
    assert!(total > 0.5, "hw.memsize gave {} GB", total);

    let free = pastor_bible_core::sidecar::free_ram_gb();
    assert!(free > 0.0, "vm_stat gave {} GB free", free);
    assert!(free <= total + 0.5, "{} GB free of {} GB total", free, total);

    let cpu = hardware::cpu_name();
    assert_ne!(cpu, "unknown", "machdep.cpu.brand_string gave nothing");

    let disk = hardware::free_disk_gb(&std::env::temp_dir().to_string_lossy());
    assert!(disk > 0.0, "statvfs gave {} GB free", disk);

    let os = hardware::os_name();
    assert!(os.starts_with("macOS"), "{}", os);

    println!("cpu {} / {:.1} GB total, {:.1} GB free / {:.1} GB disk / {}",
             cpu, total, free, disk, os);
}

/// The whole probe, against this machine's real model server, both ways.
/// Ignored by default because it needs the sidecar; run by hand and in the
/// session that changed this.
#[test]
#[ignore]
fn probe_against_the_real_server() {
    let server = pastor_bible_core::paths::llama_server();
    let app_data = std::env::temp_dir().to_string_lossy().into_owned();

    std::env::remove_var("TPB_NO_GPU");
    let with = hardware::probe(&app_data, &server, 6325);
    println!("WITH a card:  devices={} graphics={}", with.gpu_devices.len(), with.graphics);
    println!("              disk {:.1} GB on {:?}", with.free_disk_gb, with.disk_drive);

    std::env::set_var("TPB_NO_GPU", "1");
    let without = hardware::probe(&app_data, &server, 6325);
    println!("WITHOUT:      devices={} graphics={}", without.gpu_devices.len(), without.graphics);
    std::env::remove_var("TPB_NO_GPU");

    assert!(without.gpu_devices.is_empty());
    assert!(without.graphics.contains("processor"));
    assert!(!with.disk_drive.is_empty() || !cfg!(windows));
}
