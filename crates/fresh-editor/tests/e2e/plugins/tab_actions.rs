//! E2E tests for tab actions plugin

use crate::common::harness::{copy_plugin, copy_plugin_lib, EditorTestHarness};
use crate::common::harness::layout;
use crossterm::event::{KeyCode, KeyModifiers};

fn tab_actions_harness() -> (EditorTestHarness, tempfile::TempDir) {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let project_root = temp_dir.path().join("project_root");
    std::fs::create_dir_all(&project_root).unwrap();

    let plugins_dir = project_root.join("plugins");
    std::fs::create_dir_all(&plugins_dir).unwrap();
    copy_plugin(&plugins_dir, "tab_actions");
    copy_plugin_lib(&plugins_dir);

    let mut harness =
        EditorTestHarness::with_config_and_working_dir(80, 24, Default::default(), project_root)
            .unwrap();

    harness.render().unwrap();

    (harness, temp_dir)
}

#[test]
fn test_close_other_buffers() {
    let (mut harness, temp_dir) = tab_actions_harness();
    let project_root = temp_dir.path().join("project_root");

    // Create files in project root so quick open can find them
    let file1 = project_root.join("file1.txt");
    let file2 = project_root.join("file2.txt");
    let file3 = project_root.join("file3.txt");
    std::fs::write(&file1, "Content 1").unwrap();
    std::fs::write(&file2, "Content 2").unwrap();
    std::fs::write(&file3, "Content 3").unwrap();

    // Open all 3 files
    harness.open_file(&file1).unwrap();
    harness.render().unwrap();
    harness.assert_screen_contains("file1.txt");

    harness.open_file(&file2).unwrap();
    harness.render().unwrap();
    harness.assert_screen_contains("file2.txt");

    harness.open_file(&file3).unwrap();
    harness.render().unwrap();
    harness.assert_screen_contains("file3.txt");

    // Verify all 3 tabs are open
    let screen_after_opening = harness.screen_to_string();
    let file1_count = screen_after_opening.matches("file1.txt").count();
    let file2_count = screen_after_opening.matches("file2.txt").count();
    let file3_count = screen_after_opening.matches("file3.txt").count();
    assert!(
        file1_count >= 1 && file2_count >= 1 && file3_count >= 1,
        "Expected all 3 files in tabs, found file1={}, file2={}, file3={}. Screen:\n{}",
        file1_count,
        file2_count,
        file3_count,
        screen_after_opening
    );

    // Switch to file2 using Quick Open buffer mode
    harness
        .send_key(KeyCode::Char('p'), KeyModifiers::CONTROL)
        .unwrap();
    harness.render().unwrap();
    harness
        .send_key(KeyCode::Backspace, KeyModifiers::NONE)
        .unwrap();
    harness.render().unwrap();
    harness.type_text("file2").unwrap();
    harness
        .send_key(KeyCode::Enter, KeyModifiers::NONE)
        .unwrap();
    harness.render().unwrap();

    // Verify we're now on file2
    harness.assert_screen_contains("Content 2");

    // Run "Close Other Tabs" command via Ctrl+P
    harness
        .send_key(KeyCode::Char('p'), KeyModifiers::CONTROL)
        .unwrap();
    harness.render().unwrap();
    harness.type_text("Close Other Tabs").unwrap();
    harness.render().unwrap();

    // Verify the command IS visible in the palette
    let screen = harness.screen_to_string();
    assert!(
        screen.contains("Close Other Tabs"),
        "Expected 'Close Other Tabs' to be visible in command palette. Screen:\n{}",
        screen
    );

    harness
        .send_key(KeyCode::Enter, KeyModifiers::NONE)
        .unwrap();
    harness.render().unwrap();

    // Verify only file2 remains
    let screen = harness.screen_to_string();
    assert!(
        !screen.contains("file1.txt"),
        "Expected file1.txt to be closed. Screen:\n{}",
        screen
    );
    assert!(
        screen.contains("file2.txt"),
        "Expected file2.txt to remain. Screen:\n{}",
        screen
    );
    assert!(
        !screen.contains("file3.txt"),
        "Expected file3.txt to be closed. Screen:\n{}",
        screen
    );
}

#[test]
fn test_close_all_buffers() {
    let (mut harness, temp_dir) = tab_actions_harness();
    let project_root = temp_dir.path().join("project_root");

    // Create files in project root so quick open can find them
    let file1 = project_root.join("file1.txt");
    let file2 = project_root.join("file2.txt");
    let file3 = project_root.join("file3.txt");
    std::fs::write(&file1, "Content 1").unwrap();
    std::fs::write(&file2, "Content 2").unwrap();
    std::fs::write(&file3, "Content 3").unwrap();

    // Open all 3 files
    harness.open_file(&file1).unwrap();
    harness.render().unwrap();
    harness.assert_screen_contains("file1.txt");

    harness.open_file(&file2).unwrap();
    harness.render().unwrap();
    harness.assert_screen_contains("file2.txt");

    harness.open_file(&file3).unwrap();
    harness.render().unwrap();
    harness.assert_screen_contains("file3.txt");

    // Verify all 3 tabs are open
    let screen_after_opening = harness.screen_to_string();
    let file1_count = screen_after_opening.matches("file1.txt").count();
    let file2_count = screen_after_opening.matches("file2.txt").count();
    let file3_count = screen_after_opening.matches("file3.txt").count();
    assert!(
        file1_count >= 1 && file2_count >= 1 && file3_count >= 1,
        "Expected all 3 files in tabs, found file1={}, file2={}, file3={}. Screen:\n{}",
        file1_count,
        file2_count,
        file3_count,
        screen_after_opening
    );

    // Switch to file2 using Quick Open buffer mode
    harness
        .send_key(KeyCode::Char('p'), KeyModifiers::CONTROL)
        .unwrap();
    harness.render().unwrap();
    harness
        .send_key(KeyCode::Backspace, KeyModifiers::NONE)
        .unwrap();
    harness.render().unwrap();
    harness.type_text("file2").unwrap();
    harness
        .send_key(KeyCode::Enter, KeyModifiers::NONE)
        .unwrap();
    harness.render().unwrap();

    // Verify we're now on file2
    harness.assert_screen_contains("Content 2");

    // Run "Close All Tabs" command via Ctrl+P
    harness
        .send_key(KeyCode::Char('p'), KeyModifiers::CONTROL)
        .unwrap();
    harness.render().unwrap();
    harness.type_text("Close All Tabs").unwrap();
    harness.render().unwrap();

    // Verify the command IS visible in the palette
    let screen = harness.screen_to_string();
    assert!(
        screen.contains("Close All Tabs"),
        "Expected 'Close All Tabs' to be visible in command palette. Screen:\n{}",
        screen
    );

    harness
        .send_key(KeyCode::Enter, KeyModifiers::NONE)
        .unwrap();
    harness.render().unwrap();

    // Verify no files remain
    let screen = harness.screen_to_string();
    assert!(
        !screen.contains("file1.txt"),
        "Expected file1.txt to be closed. Screen:\n{}",
        screen
    );
    assert!(
        !screen.contains("file2.txt"),
        "Expected file2.txt to be closed. Screen:\n{}",
        screen
    );
    assert!(
        !screen.contains("file3.txt"),
        "Expected file3.txt to be closed. Screen:\n{}",
        screen
    );
}

#[test]
fn test_close_buffers_to_left() {
    let (mut harness, temp_dir) = tab_actions_harness();
    let project_root = temp_dir.path().join("project_root");

    // Create files in project root so quick open can find them
    let file1 = project_root.join("file1.txt");
    let file2 = project_root.join("file2.txt");
    let file3 = project_root.join("file3.txt");
    std::fs::write(&file1, "Content 1").unwrap();
    std::fs::write(&file2, "Content 2").unwrap();
    std::fs::write(&file3, "Content 3").unwrap();

    // Open all 3 files
    harness.open_file(&file1).unwrap();
    harness.render().unwrap();
    harness.assert_screen_contains("file1.txt");

    harness.open_file(&file2).unwrap();
    harness.render().unwrap();
    harness.assert_screen_contains("file2.txt");

    harness.open_file(&file3).unwrap();
    harness.render().unwrap();
    harness.assert_screen_contains("file3.txt");

    // Verify all 3 tabs are open
    let screen_after_opening = harness.screen_to_string();
    let file1_count = screen_after_opening.matches("file1.txt").count();
    let file2_count = screen_after_opening.matches("file2.txt").count();
    let file3_count = screen_after_opening.matches("file3.txt").count();
    assert!(
        file1_count >= 1 && file2_count >= 1 && file3_count >= 1,
        "Expected all 3 files in tabs, found file1={}, file2={}, file3={}. Screen:\n{}",
        file1_count,
        file2_count,
        file3_count,
        screen_after_opening
    );

    // Switch to file2 using Quick Open buffer mode
    harness
        .send_key(KeyCode::Char('p'), KeyModifiers::CONTROL)
        .unwrap();
    harness.render().unwrap();
    harness
        .send_key(KeyCode::Backspace, KeyModifiers::NONE)
        .unwrap();
    harness.render().unwrap();
    harness.type_text("file2").unwrap();
    harness
        .send_key(KeyCode::Enter, KeyModifiers::NONE)
        .unwrap();
    harness.render().unwrap();

    // Verify we're now on file2
    harness.assert_screen_contains("Content 2");

    // Run "Close Tabs To Left" command via Ctrl+P
    harness
        .send_key(KeyCode::Char('p'), KeyModifiers::CONTROL)
        .unwrap();
    harness.render().unwrap();
    harness.type_text("Close Tabs To Left").unwrap();
    harness.render().unwrap();

    // Verify the command IS visible in the palette
    let screen = harness.screen_to_string();
    assert!(
        screen.contains("Close Tabs To Left"),
        "Expected 'Close Tabs To Left' to be visible in command palette. Screen:\n{}",
        screen
    );

    harness
        .send_key(KeyCode::Enter, KeyModifiers::NONE)
        .unwrap();
    harness.render().unwrap();

    // Verify only file2 and file3 remain
    let screen = harness.screen_to_string();
    assert!(
        !screen.contains("file1.txt"),
        "Expected file1.txt to be closed. Screen:\n{}",
        screen
    );
    assert!(
        screen.contains("file2.txt"),
        "Expected file2.txt to remain. Screen:\n{}",
        screen
    );
    assert!(
        screen.contains("file3.txt"),
        "Expected file3.txt to remain. Screen:\n{}",
        screen
    );
}

#[test]
fn test_close_buffers_to_right() {
    let (mut harness, temp_dir) = tab_actions_harness();
    let project_root = temp_dir.path().join("project_root");

    // Create files in project root so quick open can find them
    let file1 = project_root.join("file1.txt");
    let file2 = project_root.join("file2.txt");
    let file3 = project_root.join("file3.txt");
    std::fs::write(&file1, "Content 1").unwrap();
    std::fs::write(&file2, "Content 2").unwrap();
    std::fs::write(&file3, "Content 3").unwrap();

    // Open all 3 files
    harness.open_file(&file1).unwrap();
    harness.render().unwrap();
    harness.assert_screen_contains("file1.txt");

    harness.open_file(&file2).unwrap();
    harness.render().unwrap();
    harness.assert_screen_contains("file2.txt");

    harness.open_file(&file3).unwrap();
    harness.render().unwrap();
    harness.assert_screen_contains("file3.txt");

    // Verify all 3 tabs are open
    let screen_after_opening = harness.screen_to_string();
    let file1_count = screen_after_opening.matches("file1.txt").count();
    let file2_count = screen_after_opening.matches("file2.txt").count();
    let file3_count = screen_after_opening.matches("file3.txt").count();
    assert!(
        file1_count >= 1 && file2_count >= 1 && file3_count >= 1,
        "Expected all 3 files in tabs, found file1={}, file2={}, file3={}. Screen:\n{}",
        file1_count,
        file2_count,
        file3_count,
        screen_after_opening
    );

    // Switch to file2 using Quick Open buffer mode
    harness
        .send_key(KeyCode::Char('p'), KeyModifiers::CONTROL)
        .unwrap();
    harness.render().unwrap();
    harness
        .send_key(KeyCode::Backspace, KeyModifiers::NONE)
        .unwrap();
    harness.render().unwrap();
    harness.type_text("file2").unwrap();
    harness
        .send_key(KeyCode::Enter, KeyModifiers::NONE)
        .unwrap();
    harness.render().unwrap();

    // Verify we're now on file2
    harness.assert_screen_contains("Content 2");

    // Run "Close Tabs To Right" command via Ctrl+P
    harness
        .send_key(KeyCode::Char('p'), KeyModifiers::CONTROL)
        .unwrap();
    harness.render().unwrap();
    harness.type_text("Close Tabs To Right").unwrap();
    harness.render().unwrap();

    // Verify the command IS visible in the palette
    let screen = harness.screen_to_string();
    assert!(
        screen.contains("Close Tabs To Right"),
        "Expected 'Close Tabs To Right' to be visible in command palette. Screen:\n{}",
        screen
    );

    harness
        .send_key(KeyCode::Enter, KeyModifiers::NONE)
        .unwrap();
    harness.render().unwrap();

    // Verify only file1 and file2 remain
    let screen = harness.screen_to_string();
    assert!(
        screen.contains("file1.txt"),
        "Expected file1.txt to remain. Screen:\n{}",
        screen
    );
    assert!(
        screen.contains("file2.txt"),
        "Expected file2.txt to remain. Screen:\n{}",
        screen
    );
    assert!(
        !screen.contains("file3.txt"),
        "Expected file3.txt to be closed. Screen:\n{}",
        screen
    );
}

#[test]
fn test_move_tab_left() {
    let (mut harness, temp_dir) = tab_actions_harness();
    let project_root = temp_dir.path().join("project_root");

    let file1 = project_root.join("file1.txt");
    let file2 = project_root.join("file2.txt");
    let file3 = project_root.join("file3.txt");
    std::fs::write(&file1, "Content 1").unwrap();
    std::fs::write(&file2, "Content 2").unwrap();
    std::fs::write(&file3, "Content 3").unwrap();

    // Open all 3 files (file1, file2, file3)
    harness.open_file(&file1).unwrap();
    harness.render().unwrap();
    harness.open_file(&file2).unwrap();
    harness.render().unwrap();
    harness.open_file(&file3).unwrap();
    harness.render().unwrap();

    // Verify initial order: file1, file2, file3
    let tab_bar = harness.screen_row_text(layout::TAB_BAR_ROW as u16);
    assert!(
        tab_bar.find("file1.txt") < tab_bar.find("file2.txt"),
        "Expected file1 before file2 in tab bar: {tab_bar}"
    );
    assert!(
        tab_bar.find("file2.txt") < tab_bar.find("file3.txt"),
        "Expected file2 before file3 in tab bar: {tab_bar}"
    );

    // Run "Move Tab Left" - file3 should move left by one
    harness
        .send_key(KeyCode::Char('p'), KeyModifiers::CONTROL)
        .unwrap();
    harness.render().unwrap();
    harness.type_text("Move Tab To Left").unwrap();
    harness.render().unwrap();

    // Verify the command IS visible in the palette
    let screen = harness.screen_to_string();
    assert!(
        screen.contains("Move Tab To Left"),
        "Expected 'Move Tab To Left' to be visible in command palette. Screen:\n{}",
        screen
    );

    harness
        .send_key(KeyCode::Enter, KeyModifiers::NONE)
        .unwrap();
    harness.render().unwrap();

    // Verify tab order changed: file1, file3, file2
    let tab_bar_after = harness.screen_row_text(layout::TAB_BAR_ROW as u16);
    assert!(
        tab_bar_after.find("file1.txt") < tab_bar_after.find("file3.txt"),
        "Expected file1 before file3 in tab bar after move: {tab_bar_after}"
    );
    assert!(
        tab_bar_after.find("file3.txt") < tab_bar_after.find("file2.txt"),
        "Expected file3 before file2 in tab bar after move: {tab_bar_after}"
    );

    // Move tab left again - file3 should swap with file1
    harness
        .send_key(KeyCode::Char('p'), KeyModifiers::CONTROL)
        .unwrap();
    harness.render().unwrap();
    harness.type_text("Move Tab Left").unwrap();
    harness
        .send_key(KeyCode::Enter, KeyModifiers::NONE)
        .unwrap();
    harness.render().unwrap();

    // Verify: file3, file1, file2
    let tab_bar_final = harness.screen_row_text(layout::TAB_BAR_ROW as u16);
    assert!(
        tab_bar_final.find("file3.txt") < tab_bar_final.find("file1.txt"),
        "Expected file3 before file1 in tab bar: {tab_bar_final}"
    );
    assert!(
        tab_bar_final.find("file1.txt") < tab_bar_final.find("file2.txt"),
        "Expected file1 before file2 in tab bar: {tab_bar_final}"
    );

    // Move tab left again - file3 is already at first, should do nothing
    harness
        .send_key(KeyCode::Char('p'), KeyModifiers::CONTROL)
        .unwrap();
    harness.render().unwrap();
    harness.type_text("Move Tab Left").unwrap();
    harness
        .send_key(KeyCode::Enter, KeyModifiers::NONE)
        .unwrap();
    harness.render().unwrap();

    // Verify order unchanged: file3, file1, file2
    let tab_bar_unchanged = harness.screen_row_text(layout::TAB_BAR_ROW as u16);
    assert!(
        tab_bar_unchanged.find("file3.txt") < tab_bar_unchanged.find("file1.txt"),
        "Expected file3 before file1 (unchanged): {tab_bar_unchanged}"
    );
    assert!(
        tab_bar_unchanged.find("file1.txt") < tab_bar_unchanged.find("file2.txt"),
        "Expected file1 before file2 (unchanged): {tab_bar_unchanged}"
    );
}

#[test]
fn test_move_tab_right() {
    let (mut harness, temp_dir) = tab_actions_harness();
    let project_root = temp_dir.path().join("project_root");

    let file1 = project_root.join("file1.txt");
    let file2 = project_root.join("file2.txt");
    let file3 = project_root.join("file3.txt");
    std::fs::write(&file1, "Content 1").unwrap();
    std::fs::write(&file2, "Content 2").unwrap();
    std::fs::write(&file3, "Content 3").unwrap();

    // Open all 3 files (file1, file2, file3)
    harness.open_file(&file1).unwrap();
    harness.render().unwrap();
    harness.open_file(&file2).unwrap();
    harness.render().unwrap();
    harness.open_file(&file3).unwrap();
    harness.render().unwrap();

    // Verify initial order: file1, file2, file3
    let tab_bar = harness.screen_row_text(layout::TAB_BAR_ROW as u16);
    assert!(
        tab_bar.find("file1.txt") < tab_bar.find("file2.txt"),
        "Expected file1 before file2 in tab bar: {tab_bar}"
    );
    assert!(
        tab_bar.find("file2.txt") < tab_bar.find("file3.txt"),
        "Expected file2 before file3 in tab bar: {tab_bar}"
    );

    // Run "Move Tab Right" - file3 is at last position, should do nothing
    harness
        .send_key(KeyCode::Char('p'), KeyModifiers::CONTROL)
        .unwrap();
    harness.render().unwrap();
    harness.type_text("Move Tab To Right").unwrap();
    harness.render().unwrap();

    // Verify the command IS visible in the palette
    let screen = harness.screen_to_string();
    assert!(
        screen.contains("Move Tab To Right"),
        "Expected 'Move Tab To Right' to be visible in command palette. Screen:\n{}",
        screen
    );

    harness
        .send_key(KeyCode::Enter, KeyModifiers::NONE)
        .unwrap();
    harness.render().unwrap();

    // Verify order unchanged: file1, file2, file3
    let tab_bar_after = harness.screen_row_text(layout::TAB_BAR_ROW as u16);
    assert!(
        tab_bar_after.find("file1.txt") < tab_bar_after.find("file2.txt"),
        "Expected file1 before file2 (unchanged): {tab_bar_after}"
    );
    assert!(
        tab_bar_after.find("file2.txt") < tab_bar_after.find("file3.txt"),
        "Expected file2 before file3 (unchanged): {tab_bar_after}"
    );

    // Switch to file1 (first tab)
    harness
        .send_key(KeyCode::Char('p'), KeyModifiers::CONTROL)
        .unwrap();
    harness.render().unwrap();
    harness
        .send_key(KeyCode::Backspace, KeyModifiers::NONE)
        .unwrap();
    harness.render().unwrap();
    harness.type_text("file1").unwrap();
    harness
        .send_key(KeyCode::Enter, KeyModifiers::NONE)
        .unwrap();
    harness.render().unwrap();

    // Move tab right - file1 should move to position 2
    // After move: file2, file1, file3
    harness
        .send_key(KeyCode::Char('p'), KeyModifiers::CONTROL)
        .unwrap();
    harness.render().unwrap();
    harness.type_text("Move Tab Right").unwrap();
    harness
        .send_key(KeyCode::Enter, KeyModifiers::NONE)
        .unwrap();
    harness.render().unwrap();

    let tab_bar_move1 = harness.screen_row_text(layout::TAB_BAR_ROW as u16);
    assert!(
        tab_bar_move1.find("file2.txt") < tab_bar_move1.find("file1.txt"),
        "Expected file2 before file1 after move: {tab_bar_move1}"
    );
    assert!(
        tab_bar_move1.find("file1.txt") < tab_bar_move1.find("file3.txt"),
        "Expected file1 before file3 after move: {tab_bar_move1}"
    );

    // Move tab right again - file1 should move to last position
    // After move: file2, file3, file1
    harness
        .send_key(KeyCode::Char('p'), KeyModifiers::CONTROL)
        .unwrap();
    harness.render().unwrap();
    harness.type_text("Move Tab Right").unwrap();
    harness
        .send_key(KeyCode::Enter, KeyModifiers::NONE)
        .unwrap();
    harness.render().unwrap();

    let tab_bar_move2 = harness.screen_row_text(layout::TAB_BAR_ROW as u16);
    assert!(
        tab_bar_move2.find("file2.txt") < tab_bar_move2.find("file3.txt"),
        "Expected file2 before file3 after second move: {tab_bar_move2}"
    );
    assert!(
        tab_bar_move2.find("file3.txt") < tab_bar_move2.find("file1.txt"),
        "Expected file3 before file1 after second move: {tab_bar_move2}"
    );
}
