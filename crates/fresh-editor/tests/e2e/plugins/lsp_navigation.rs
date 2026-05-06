//! E2E tests for lsp_navigation plugin

use crate::common::harness::{copy_plugin, copy_plugin_lib, EditorTestHarness};
use crossterm::event::{KeyCode, KeyModifiers};
use std::fs;

/// Test LSP navigation functionality with a fake LSP server
///
/// This test verifies that the lsp_navigation plugin works correctly:
/// 1. LSP server responds to textDocument/documentSymbol
/// 2. The lsp_navigation plugin receives the results
/// 3. The symbols are displayed in the command palette with correct labels
#[test]
#[cfg_attr(windows, ignore)] // Uses bash script for fake LSP server
fn test_lsp_navigation_symbols() -> anyhow::Result<()> {
    let temp_dir = tempfile::TempDir::new()?;
    let project_root = temp_dir.path().to_path_buf();

    let plugins_dir = project_root.join("plugins");
    fs::create_dir(&plugins_dir)?;

    copy_plugin(&plugins_dir, "lsp_navigation");
    copy_plugin_lib(&plugins_dir);

    let fake_lsp_script = r#"#!/bin/bash

read_message() {
    local content_length=0
    while IFS=: read -r key value; do
        key=$(echo "$key" | tr -d '\r\n')
        value=$(echo "$value" | tr -d '\r\n ')
        if [ "$key" = "Content-Length" ]; then
            content_length=$value
        fi
        if [ -z "$key" ]; then
            break
        fi
    done

    if [ $content_length -gt 0 ]; then
        dd bs=1 count=$content_length 2>/dev/null
    fi
}

send_message() {
    local message="$1"
    local length=${#message}
    echo -en "Content-Length: $length\r\n\r\n$message"
}

while true; do
    msg=$(read_message)

    if [ -z "$msg" ]; then
        break
    fi

    method=$(echo "$msg" | grep -o '"method":"[^"]*"' | cut -d'"' -f4)
    msg_id=$(echo "$msg" | grep -o '"id":[0-9]*' | cut -d':' -f2)

    case "$method" in
        "initialize")
            send_message '{"jsonrpc":"2.0","id":'$msg_id',"result":{"capabilities":{"documentSymbolProvider":true,"textDocumentSync":1}}}'
            ;;
        "initialized")
            ;;
        "textDocument/didOpen"|"textDocument/didChange"|"textDocument/didSave")
            ;;
        "textDocument/documentSymbol")
            send_message '{"jsonrpc":"2.0","id":'$msg_id',"result":[{"name":"MyClass","kind":5,"location":{"uri":"file://test.ts","range":{"start":{"line":0,"character":0},"end":{"line":8,"character":1}}}},{"name":"constructor","kind":9,"location":{"uri":"file://test.ts","range":{"start":{"line":1,"character":2},"end":{"line":3,"character":3}}}},{"name":"myMethod","kind":6,"location":{"uri":"file://test.ts","range":{"start":{"line":5,"character":2},"end":{"line":7,"character":3}}}}]}'
            ;;
        "shutdown")
            send_message '{"jsonrpc":"2.0","id":'$msg_id',"result":null}'
            break
            ;;
    esac
done
"#;

    let script_path = project_root.join("fake_lsp.sh");
    fs::write(&script_path, fake_lsp_script)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms)?;
    }

    let test_file = project_root.join("test.ts");
    fs::write(
        &test_file,
        r#"class MyClass {
  constructor() {
    return true;
  }

  myMethod(a: number): number {
    return a;
  }
}
"#,
    )?;

    let mut config = fresh::config::Config::default();
    config.lsp.insert(
        "typescript".to_string(),
        fresh::types::LspLanguageConfig::Multi(vec![fresh::services::lsp::LspServerConfig {
            command: script_path.to_string_lossy().to_string(),
            args: vec![],
            enabled: true,
            auto_start: true,
            process_limits: fresh::services::process_limits::ProcessLimits::default(),
            initialization_options: None,
            env: Default::default(),
            language_id_overrides: Default::default(),
            root_markers: Default::default(),
            name: None,
            only_features: None,
            except_features: None,
        }]),
    );

    let mut harness =
        EditorTestHarness::with_config_and_working_dir(100, 30, config, project_root.clone())?;

    harness.open_file(&test_file)?;
    harness.process_async_and_render()?;

    for _ in 0..5 {
        harness.process_async_and_render()?;
    }

    harness.send_key(KeyCode::Char('p'), KeyModifiers::CONTROL)?;
    harness.process_async_and_render()?;
    harness.type_text("Go to LSP Symbol")?;
    harness.send_key(KeyCode::Enter, KeyModifiers::NONE)?;

    harness.wait_for_prompt()?;
    harness.render()?;

    harness.wait_until(|h| {
        let screen = h.screen_to_string();
        screen.contains("[class] MyClass")
            || screen.contains("[construct] constructor")
            || screen.contains("[method] myMethod")
    })?;

    let screen = harness.screen_to_string();

    assert!(
        screen.contains("[class] MyClass"),
        "Screen should contain '[class] MyClass'. Screen:\n{}",
        screen
    );
    assert!(
        screen.contains("[construct] constructor"),
        "Screen should contain '[construct] constructor'. Screen:\n{}",
        screen
    );
    assert!(
        screen.contains("[method] myMethod"),
        "Screen should contain '[method] myMethod'. Screen:\n{}",
        screen
    );

    for _ in 0..3 {
        harness.process_async_and_render()?;
    }

    let selection = harness.get_selection_range();
    assert!(
        selection.is_some(),
        "After selecting first symbol (MyClass), editor should have a selection. Screen:\n{}",
        harness.screen_to_string()
    );
    let selected_text = harness.get_selected_text();
    let screen = harness.screen_to_string();
    assert!(
        selected_text == "class MyClass {\n  constructor() {\n    return true;\n  }\n\n  myMethod(a: number): number {\n    return a;\n  }\n}",
        "First item (MyClass) should select full class. Selected text: {:?}\nScreen:\n{}",
        selected_text,
        screen
    );

    harness.send_key(KeyCode::Down, KeyModifiers::NONE)?;
    for _ in 0..3 {
        harness.process_async_and_render()?;
    }

    let selection = harness.get_selection_range();
    assert!(
        selection.is_some(),
        "After selecting constructor, editor should have a selection. Screen:\n{}",
        harness.screen_to_string()
    );
    let selected_text = harness.get_selected_text();
    let screen = harness.screen_to_string();
    assert!(
        selected_text == "  constructor() {\n    return true;\n  }",
        "constructor should be selected. Selected text: {:?}\nScreen:\n{}",
        selected_text,
        screen
    );

    harness.send_key(KeyCode::Down, KeyModifiers::NONE)?;
    for _ in 0..3 {
        harness.process_async_and_render()?;
    }

    let selection = harness.get_selection_range();
    assert!(
        selection.is_some(),
        "After selecting myMethod, editor should have a selection. Screen:\n{}",
        harness.screen_to_string()
    );
    let selected_text = harness.get_selected_text();
    let screen = harness.screen_to_string();
    assert!(
        selected_text == "  myMethod(a: number): number {\n    return a;\n  }",
        "myMethod should be selected. Selected text: {:?}\nScreen:\n{}",
        selected_text,
        screen
    );

    Ok(())
}
