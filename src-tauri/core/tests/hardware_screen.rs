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
fn a_card_big_enough_says_it_will_be_used() {
    // This machine: 10 GB card with room.
    let s = hardware::graphics_sentence(&[dev("NVIDIA GeForce RTX 3080", 10267, 9495)], 6325);
    assert!(s.contains("RTX 3080"), "{}", s);
    assert!(s.contains("big enough"), "{}", s);
    assert!(!s.contains("too small"), "{}", s);
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
    } else {
        assert_eq!(hardware::drive_of("/home/someone"), "/");
    }
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
