//! The low-memory refusal, in words a reader can act on.
//!
//! P7's laptop was told "needs 6.7 GB free, only 4.3 GB available". The reader
//! took it for disk space: they uninstalled programs looking for room, and it
//! was a reboot that finally fixed it, because it was memory all along.
//!
//! These tests assert on `low_memory_message`, which is the exact string the
//! refusal returns, rather than driving `Sidecar::start`. The first version did
//! drive it, and set `TPB_FAKE_FREE_RAM_GB` to force the failure. That passed
//! here under `--test-threads=1` and failed in CI, where tests in one binary
//! share a process and run in parallel: they raced each other's environment
//! variable and tripped the one-sidecar-at-a-time rule. A test that only passes
//! when it has the process to itself is a test with a hidden precondition.

use pastor_bible_core::sidecar::{free_ram_gb, low_memory_message, Role};

#[test]
fn the_refusal_says_memory_and_what_to_do() {
    // The laptop's own numbers.
    let m = low_memory_message(6.7, 4.3, Role::Chat);
    let lower = m.to_lowercase();

    assert!(lower.contains("free memory"), "must say memory: {}", m);
    assert!(
        !lower.contains("space") && !lower.contains("disk"),
        "must never say space or disk, which is what the laptop reader heard: {}",
        m
    );
    assert!(m.contains("6.7 GB"), "must say how much is needed: {}", m);
    assert!(m.contains("4.3 GB"), "must say how much there is: {}", m);
    assert!(
        lower.contains("close other programs") && lower.contains("restart the computer"),
        "must say what to do about it: {}",
        m
    );
    assert!(
        lower.contains("try again"),
        "must say the button is worth pressing again: {}",
        m
    );
}

#[test]
fn it_names_the_model_the_way_settings_does() {
    assert!(low_memory_message(6.7, 4.3, Role::Chat).contains("the answering model"));
    assert!(low_memory_message(1.0, 0.5, Role::Embedding).contains("the search model"));
}

#[test]
fn no_file_path_is_shown_to_the_reader() {
    let m = low_memory_message(6.7, 4.3, Role::Chat);
    let backslash = char::from(92);
    assert!(!m.contains(backslash) && !m.contains(".gguf"), "a path leaked in: {}", m);
}

/// Two readings produce two different messages, so a reader who closes
/// something and presses again is told a new number rather than the old one.
#[test]
fn a_new_reading_produces_a_new_message() {
    let first = low_memory_message(6.7, 0.1, Role::Chat);
    let second = low_memory_message(6.7, 0.9, Role::Chat);
    assert!(first.contains("0.1 GB"), "{}", first);
    assert!(second.contains("0.9 GB"), "{}", second);
    assert_ne!(first, second);
}

/// Free memory is read from the OS at the moment it is asked for, which is what
/// makes pressing the button again a real retry rather than a repeat of a
/// cached answer. `TPB_FAKE_FREE_RAM_GB` is how the low-memory path is
/// exercised without filling a machine's memory, as `TPB_NO_GPU` is for
/// graphics.
///
/// This is the only test here that touches the environment, so nothing else in
/// this binary can race it.
#[test]
fn free_memory_is_read_when_it_is_asked_for() {
    std::env::set_var("TPB_FAKE_FREE_RAM_GB", "4.3");
    assert!((free_ram_gb() - 4.3).abs() < 0.001);

    // A different answer without anything else changing: the value is not
    // cached from the first call.
    std::env::set_var("TPB_FAKE_FREE_RAM_GB", "0.9");
    assert!((free_ram_gb() - 0.9).abs() < 0.001);

    std::env::remove_var("TPB_FAKE_FREE_RAM_GB");
    assert!(free_ram_gb() > 0.0, "the real reading should be a real number");
}
