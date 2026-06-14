/// <reference path="./lib/fresh.d.ts" />
//
// Test-only probe plugin. Pops an action popup carrying a unique marker
// every time the host fires the `after_file_open` hook. Used to assert
// that previewing a file (file-explorer browse) does NOT fire the hook —
// mirroring how the real asm-lsp helper raises its `.asm-lsp.toml`
// config-offer popup from `after_file_open`.
const editor = getEditor();

editor.on("after_file_open", (data) => {
  editor.showActionPopup({
    id: "after-open-probe",
    title: "AFTER_OPEN_PROBE_MARKER",
    message: `after_file_open fired for ${data.path}`,
    actions: [{ id: "ok", label: "OK" }],
  });
});

editor.debug("test_after_open_popup: loaded");
