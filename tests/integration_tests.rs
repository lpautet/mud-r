//! Integration tests for the MUD-R project
//! These tests verify that different components work together correctly

use std::process::Command;

#[test]
fn test_help_output() {
    let output = Command::new("cargo")
        .args(&["run", "--", "--help"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    // Verify help contains expected flags
    assert!(stdout.contains("--check"));
    assert!(stdout.contains("--dir"));
    assert!(stdout.contains("--mini"));
    assert!(stdout.contains("--quick"));
    assert!(stdout.contains("--restrict"));
    assert!(stdout.contains("--no-specials"));
}

#[test]
fn test_syntax_check_mode() {
    let output = Command::new("cargo")
        .args(&["run", "--", "--check"])
        .output()
        .expect("Failed to execute command");

    // In check mode, the program should exit early
    // We expect it to either succeed or fail gracefully
    let stderr = String::from_utf8_lossy(&output.stderr);
    
    // Should not crash with panic
    assert!(!stderr.contains("panic"));
    assert!(!stderr.contains("GURU MEDITATION"));
}

#[test]
fn test_invalid_port_handling() {
    let output = Command::new("cargo")
        .args(&["run", "--", "100"]) // Port < 1024 should be rejected
        .output()
        .expect("Failed to execute command");

    let stderr = String::from_utf8_lossy(&output.stderr);
    
    // Should handle invalid port gracefully
    assert!(!stderr.contains("panic"));
}

#[test]
fn test_directory_flag() {
    let output = Command::new("cargo")
        .args(&["run", "--", "--directory", "/tmp", "--check"])
        .output()
        .expect("Failed to execute command");

    let stderr = String::from_utf8_lossy(&output.stderr);
    
    // Should handle directory flag without crashing
    assert!(!stderr.contains("panic"));
}
