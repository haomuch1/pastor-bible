//! The low-memory refusal, in words a reader can act on, and re-measured on
//! every attempt.
//!
//! P7's laptop was told "needs 6.7 GB free, only 4.3 GB available". The reader
//! took that for disk space: they uninstalled programs looking for room, and
//! it was a reboot that finally fixed it, because it was free memory all
//! along. Then pressing the button again appeared to do nothing.
//!
//! `TPB_FAKE_FREE_RAM_GB` exists so this path can be exercised without filling
//! a machine's memory for real, the way `TPB_NO_GPU` exercises the other one.

use pastor_bible_core::sidecar::{free_ram_gb, Options, Role, Sidecar};

/// A real file to measure, so the size check passes and the memory check is
/// what fails. The search model ships with the installer and is always here.
fn a_model() -> String {
    pastor_bible_core::paths::embed_model()
}

#[test]
fn the_reported_free_memory_can_be_forced() {
    std::env::set_var("TPB_FAKE_FREE_RAM_GB", "4.3");
    assert!((free_ram_gb() - 4.3).abs() < 0.001);
    std::env::remove_var("TPB_FAKE_FREE_RAM_GB");
    assert!(free_ram_gb() > 0.0, "the real reading should be a real number");
}

#[test]
fn the_refusal_says_memory_and_what_to_do() {
    let model = a_model();
    if !std::path::Path::new(&model).exists() {
        eprintln!("skipping: {} is not here", model);
        return;
    }
    std::env::set_var("TPB_FAKE_FREE_RAM_GB", "0.1");
    let mut opts = Options::new(&pastor_bible_core::paths::llama_server(), &model, Role::Chat);
    opts.headroom_gb = 2.0;
    let err = Sidecar::start(&opts).err().expect("it must refuse");
    std::env::remove_var("TPB_FAKE_FREE_RAM_GB");

    let lower = err.to_lowercase();
    assert!(lower.contains("memory"), "must say memory: {}", err);
    assert!(
        !lower.contains("space") && !lower.contains("disk"),
        "must never say space or disk, which is what the laptop reader heard: {}",
        err
    );
    assert!(lower.contains("0.1 gb free"), "must give what is free now: {}", err);
    assert!(
        lower.contains("close other programs") && lower.contains("restart"),
        "must say what to do: {}",
        err
    );
    assert!(
        lower.contains("answering model"),
        "must name what it was loading, as Settings does: {}",
        err
    );
    assert!(!err.contains(&model), "the reader is not shown a file path: {}", err);
}

/// The check reads free memory at the moment it is made, so pressing the
/// button again after closing something really does try again.
#[test]
fn every_attempt_measures_again() {
    let model = a_model();
    if !std::path::Path::new(&model).exists() {
        return;
    }
    let mut opts = Options::new(&pastor_bible_core::paths::llama_server(), &model, Role::Chat);
    opts.headroom_gb = 2.0;

    std::env::set_var("TPB_FAKE_FREE_RAM_GB", "0.1");
    let first = Sidecar::start(&opts).err().expect("refused while memory is short");
    assert!(first.to_lowercase().contains("0.1 gb free"));

    // The reader closes something. Nothing else changes.
    std::env::set_var("TPB_FAKE_FREE_RAM_GB", "0.9");
    let second = Sidecar::start(&opts).err().expect("still short, but by less");
    std::env::remove_var("TPB_FAKE_FREE_RAM_GB");

    assert!(
        second.to_lowercase().contains("0.9 gb free"),
        "the second attempt reused the first reading instead of measuring again: {}",
        second
    );
    assert_ne!(first, second, "two attempts gave byte-identical answers");
}
