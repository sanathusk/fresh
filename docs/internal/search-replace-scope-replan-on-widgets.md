# Search & Replace scope feature — replan on top of the widget runtime

> **Status**: planning. Supersedes
> `docs/internal/search-replace-ux-improvements.md` for the scope
> subset (everything that was done on `claude/search-replace-improvements-WUcez`
> against the pre-widget plugin). The wider UX spec still applies for
> sections that already landed on master through the widget migration.
> **Date**: 2026-05-08
> **Branch**: `claude/search-replace-improvements-WUcez` (history retained
> for context; reland deltas as fresh commits on top of master)

## Why this exists

A 27-commit branch shipped an end-to-end pass at the search/replace
UX while master simultaneously rewrote the same plugin onto a brand-
new widget runtime (51 widget-themed commits, see
`docs/internal/plugin-widget-library-design.md`). Both forks touched
`crates/fresh-editor/plugins/search_replace.ts` in incompatible ways:

- **Branch:** hand-rolled `TextPropertyEntry` rendering, byte-offset
  cursor math in TS, `setVirtualBufferContent` per keystroke,
  in-plugin Tab cycling, hand-rolled per-file checkbox glyph, etc.
- **Master:** the same widgets expressed declaratively
  (`textInput`, `toggle`, `button`, `tree` with `checkable: true`,
  `hintBar`); host-owned cursor, focus, hit-test, scroll; plugin
  reacts to `widget_event` instead of computing layout.

A `git rebase` produces conflicts on every line of every file
overlap. The right move is not to re-litigate them — it is to ask,
*per UX behaviour the branch added*, whether the same behaviour now
lives in master via the widget rewrite or whether the gap survived.
Then reland only the survivors, in the widget idiom, as fresh
commits.

Note: this doc is about the **scope-and-related search/replace UX**
(All-Files toggle, Current-File command, history, multi-line,
empty-state quality, panel passthrough, scroll-to-view, default
keybinding). It is **not** about the widget library — that has its
own design doc.

## What the branch actually did

For reference, the 27 commits on
`claude/search-replace-improvements-WUcez` map to UX sections of
`docs/internal/search-replace-ux-improvements.md` as follows:

| Section | Behaviour shipped on branch |
|---|---|
| §1 | `allFiles` panel state + `start_search_replace_in_buffer` palette command + post-filter on grep results + dynamic panel title (`*Search/Replace (a.txt)*`) + visible `[v] All Files` toolbar toggle + `Searching in: <filename>` row + Alt+A binds to scope toggle inside panel + actions row with `Replace Selected (N)` / `Replace All in <filename>` |
| §2 | Tab/Shift-Tab cycles every toolbar control (5 stops) |
| §3 | `setActiveFieldText` → `rerunSearchDebounced()` on every search-field mutation |
| §4 | Three-state per-file checkbox `[v]/[~]/[ ]` + click-to-jump on match rows (panel-mounted `mouse_click` listener mapping `buffer_row - matchesStartRow + scrollOffset` to a flat-item index) |
| §5 | `Editor::ensure_active_cursor_visible_for_navigation` extended to set `viewport.left_column` so a match deep inside a long line is visible |
| §10 | Keymap: `Alt+A` (normal-context) → `start_search_replace` |
| §11 | In-memory 20-entry search history; Up/Down arrows walk it; saved-pattern snapshot on entry |
| §14 | `Alt+J` inserts `\n` in either input; field grows vertically with continuation rows; cursor up/down/home/end work per-row; `TextArea` factored to `plugins/lib/text_area.ts` |
| §15 | `panel.help` i18n rewritten (`Tab: next  Space: include/exclude  Enter: open  Alt+Ret: replace selected  Esc: close`) across 12 locales |
| §17 | `panel.searchPerformed: boolean` flag; empty-state branches render `Type a search pattern above` / `Searching…` / `No matches found` distinctly; new `panel.searching` i18n key |
| §18 | `is_panel_passthrough_action` whitelist in `keybindings.rs` + Mode-context fallthrough check so `Alt+]` / `Ctrl+S` / `Ctrl+P` work while a plugin panel is focused |

## What master already gives us

The widget rewrite delivered, sometimes incidentally, several items
the branch was about. Confirmed by reading `origin/master`'s
`search_replace.ts` and `keybindings.rs`:

| Branch §| In master? | How |
|---|---|---|
| 2 (Tab cycle) | **Yes** | Host-driven focus walks through every widget; `dispatch(widgetKey("Tab"))` |
| 3 (search-on-type) | **Yes** | `widget_event "change"` handler on `searchField` calls `rerunSearchDebounced()` (commit `608ce6b29`) |
| 4 (per-row checkbox + click-to-jump) | **Yes** | `tree({ checkable: true })`; per-row `[v]`/`[ ]` glyph + Space-toggles-on-focused-row (commits `e2835a0da`, `a4a7b5331`); click on a row fires `select`, second click / Enter fires `activate` |
| 6, 7, 8, 9, 16 | **Yes** (pre-branch) | Already worked |
| 15 (footer hint i18n) | **No** | `panel.help` is still the old `Tab:section ↑↓:nav Space:toggle Enter:confirm Alt+Ret:replace Shift+Ret:focused Esc:close` |
| 17 (empty-state quality) | **No** | Stats label still uses `(searchPattern ? "no_matches" : "")` — prematurely claims "no matches" the moment the user types one character |
| 18 (panel passthrough) | **No** | `KeyContext::allows_ui_fallthrough` returns true only for `FileExplorer`. Mode contexts still swallow `Alt+]`, `Ctrl+S`, … |
| 10 (Alt+A binding) | **No** | No global keybinding opens the panel |
| 1 (scope feature) | **No** | No `allFiles` state, no Current-File command, no scope toggle in toolbar, no scope row, no actions-row label change |
| 5 (horizontal scroll) | **No** | `navigation.rs` doesn't touch `viewport.left_column` |
| 11 (history) | **No** | No `searchHistory` array; Up/Down do not walk a history |
| 14 (multi-line) | **Partial** | Widget runtime has a unified `Text` widget with a `rows` parameter (and a `textArea` builder); `search_replace.ts` mounts a `textInput` (rows=1). Plugin-side multi-line is a one-line opt-in; the actual editing model is host-owned now |

## What we still need (replan)

Six tickets. Each is "redo on top of master in the widget idiom" —
not "lift my old patch". Drop the per-§ rationale and prose from the
original UX doc; it's all true and unchanged. What follows is the
*implementation shape* on the new substrate, in priority order.

### 1. Empty-state quality (§17)

**User-visible:** Don't say "No matches found" the moment a user
types a single character. Distinguish *pristine* / *searching now* /
*no results* / *pattern set, no search yet*. Keep stale results
visible during the brief edit window so the list doesn't flicker.

**Shape on master:**

- Plugin: add `searchPerformed: boolean` to `PanelState` (set true at
  end of `rerunSearch` / `rerunSearchQuiet`, set false in the
  `widget_event "change"` handler for `searchField`, also reset on
  scope-toggle or pattern-changing toggle).
- Plugin: in `buildSpec`, replace the line-1 stats string and the
  matches-area `Raw` empty-state row with the same four-branch
  rendering the branch had. The matches-area row is still a
  `raw(...)` placeholder; the line-1 stats text just drops
  "no_matches" unless `searchPerformed && !busy`.
- i18n: add `panel.searching` in all 12 locales (English: `Searching…`).

**Cost:** small. ~40 LoC in the plugin + 12 i18n entries.

**Tests (e2e):** type a pattern without confirming → asserts neither
"No matches" nor the in-list placeholder appears prematurely;
mutate after a no-match search → asserts the label clears.

### 2. Panel passthrough (§18)

**User-visible:** `Alt+]` / `Ctrl+PageUp` / `Ctrl+S` / `Ctrl+P` /
`Ctrl+Q` keep working while focus is in the search/replace panel.

**Shape on master:** the right knob already exists. Two-line change:

- `KeyContext::allows_ui_fallthrough()` in
  `crates/fresh-editor/src/input/keybindings.rs` adds `Mode(_)` to
  the match. The existing resolver branch (lines ~1838-1872) already
  consults `is_terminal_ui_action` when this returns true, so the
  whole pass-through set (split nav, palette, save, quit, file
  explorer toggle, …) lights up for plugin modes automatically. No
  new whitelist needed.
- Optional: add a unit test in `keybindings.rs::tests` mirroring the
  branch's `test_panel_passthrough_for_global_navigation` — proves
  `Alt+]` from `Mode("search-replace-list")` resolves to `NextSplit`
  and `Ctrl+D` (an editing action) does **not**.

**Cost:** trivially small. Single-line code change + one unit test.

**Risk:** opens up *every* plugin mode to UI-fallthrough actions.
Audit: are there plugin modes (e.g. terminal escape sequences,
modal Vim editors) that explicitly want to swallow `Alt+]`? Per the
existing `is_terminal_ui_action` curation, the answer is "the
whitelist is already conservative — split/tab nav, palette, save,
help — none of which a sensible mode would want to suppress." Ship.

### 3. Footer hint rewrite (§15)

**User-visible:** `Tab:section ↑↓:nav Space:toggle Enter:confirm Alt+Ret:replace Shift+Ret:focused Esc:close` is wrong on three counts (terminals can't distinguish Shift+Enter from Enter; Enter doesn't "confirm"; Tab no longer just walks "sections"). New text:

```
Tab: next  Space: include/exclude  Enter: open  Alt+Ret: replace selected  Esc: close
```

**Shape on master:** edit `panel.help` in all 12 locales of
`search_replace.i18n.json`. The plugin already calls
`parseHintString(t("panel.help"))` and pushes it through the
`HintBar` widget — no plugin code change.

**Cost:** trivial. 12 string updates.

### 4. Scope feature (§1 — the actual ask)

**User-visible:**

- New palette command "Search and Replace in Current File"
  (`start_search_replace_in_buffer`).
- Toolbar gets a leading `[v] All Files` toggle (mnemonic `Alt+A` inside the panel) before Case / Regex / Whole.
- When `allFiles=false`, a dim `Searching in: <filename>` row appears
  between the toggles and the actions row.
- Action button label tracks scope + selection count:
  - All-files: `Replace Selected (N)` (dim when N=0).
  - Single-file: `Replace All in <filename>`.
- Panel title bar: `*Search/Replace*` in all-files mode,
  `*Search/Replace (<filename>)*` in single-file mode.
- Title and labels update live when the toggle flips.

**Shape on master:**

- `PanelState` gains `allFiles: boolean` (default `true`) and
  `sourceBufferPath: string` (captured in `openPanel()` via
  `editor.getBufferPath(editor.getActiveBufferId())`).
- New `start_search_replace_in_buffer` handler that calls
  `openPanel({ allFiles: false })`. Registered through
  `editor.registerCommand(...)` with a new pair of i18n strings
  (`cmd.search_replace_in_buffer{,_desc}`).
- `openPanel` signature gains an `opts?: { allFiles?: boolean }`
  parameter; falls back to project mode automatically when the
  source buffer has no associated file path (unnamed scratch buffer
  → empty path → can't filter, so `allFiles=true`).
- In `buildSpec`, the options row gains a new leading toggle:
  ```
  toggle("allFiles", t("panel.all_files_toggle"), panel.allFiles)
  ```
  The toolbar focus walk picks it up automatically (widget runtime
  manages focus; no `TOOLBAR_CONTROL_COUNT` constant to update).
- `widget_event "toggle"` handler routes `widget_key === "allFiles"`
  through a new `setScope(allFiles)` that:
  - Updates `panel.allFiles`, clears `searchResults` / `fileGroups`,
    triggers `rerunSearchDebounced`.
  - Updates the panel buffer name via `editor.renameBuffer` (or the
    equivalent for the virtual panel buffer — see implementation
    note below).
  - Calls `panel.set(buildSpec())` to swap the toggle visual state.
- In single-file mode, the streaming grep callback drops matches
  whose `match.file` !== `panel.sourceBufferPath`. Backend stays
  untouched for v1 (we eat the wasted scan; cheap enough for
  one-shot use; a `pathFilter` argument to `grepProjectStreaming`
  is a follow-up optimization).
- The "Searching in:" row is a `raw(...)` entry rendered only when
  `!allFiles && sourceBufferPath`.
- The actions row's button:
  - In all-files mode, label is `t("panel.replace_selected_btn", { count })`.
  - In single-file mode, label is `t("panel.replace_all_in_file_btn", { file: baseName(sourceBufferPath) })`.
  - The existing `doReplaceAll` already filters by
    `r.selected`, so semantics are unchanged; only the label moves.
- Panel title: `mountWidgetPanel` accepts a `name` already; refresh
  it on scope flip. If the host doesn't currently expose a rename
  primitive for the panel buffer, that's the one missing host piece —
  add `WidgetPanel.setTitle(name)` on the IPC.
- New i18n keys (12 locales): `cmd.search_replace_in_buffer`,
  `cmd.search_replace_in_buffer_desc`, `panel.all_files_toggle`,
  `panel.searching_in`, `panel.replace_selected_btn`,
  `panel.replace_all_in_file_btn`, `status.no_source_file`.

**Cost:** medium. The biggest behavioural piece on the list. ~150
LoC of plugin code + ~70 i18n entries + possibly a small host IPC
for `setTitle` (otherwise we recreate the panel on scope flip — ugly,
preserves instance state across rerender per the widget plan but
loses the title until the next mount cycle).

**Tests (e2e):**
- Open via palette → assert `*Search/Replace*`.
- Open via "Current File" command → assert `*Search/Replace (alpha.txt)*`,
  `[ ] All Files` toggle, `Searching in: alpha.txt` row, and that a
  search for "hello" doesn't surface `beta.txt` matches.
- Flip the toggle mid-session → assert title and label update.
- Action-button label tracks count: type "hello" → wait for
  results → label reads `Replace Selected (N)` with N>0.

### 5. Default keybinding (§10)

**User-visible:** `Alt+A` opens "Search and Replace in Project" from
anywhere in the editor.

**Shape on master:**

- One entry in `crates/fresh-editor/keymaps/default.json`:
  ```json
  { "key": "a", "modifiers": ["alt"], "action": "start_search_replace",
    "when": "normal" }
  ```
- Verify the panel mode's Alt+A (scope-toggle) doesn't shadow it.
  With the widget runtime, the panel-internal Alt+A flows through
  the widget's `key` action handler, which is dispatched only when
  the widget has focus; the global binding fires only when focus is
  outside the panel mode. (If conflict arises in practice, scope the
  global to `when: "normal"` and accept the panel-context override —
  it's what the branch did.)

**Cost:** trivial. Two-line keymap edit.

### 6. Multi-line input (§14)

**User-visible:** Press `Alt+J` in either input field to insert a
literal `\n`; the field grows vertically; the search engine sees the
actual newline so multi-line literal/regex patterns work.

**Shape on master:** much simpler than the branch's version, because
the unified `Text` widget already exists:

- Swap the two `textInput(...)` calls in `search_replace.ts` for
  `text({ value, rows: 5, multiline: true, ... })` (or
  `textArea({ value, rows: 5, ... })` — same widget under the hood).
- The widget already supports per-row cursor up/down, line-relative
  Home/End, Enter-inserts-newline. We need Alt+J specifically
  *in addition to* Enter-inserts-newline because the user requested
  it — but with multi-line mode on, Enter already does the job, and
  Alt+J becomes optional polish. Drop Alt+J from the v1 scope.

- Backend regex: when `searchPattern.includes("\n") && useRegex`,
  prepend `(?s)` so `.` matches newlines. Tiny patch in
  `crates/fresh-editor/src/buffer/.../search_scan_*` (or wherever
  the regex compiler is called). For literal-mode (`fixedString:
  true`) the engine already handles multi-line bytes correctly.
- Multi-line match context: today `match.context` is single-line; a
  match spanning lines needs the renderer to emit multiple `Raw`
  rows or a `↵` collapse. Defer unless users ask.

**Cost:** small at the plugin layer (one builder swap). Medium for
the regex backend tweak (needs a careful test on a multi-line
pattern). Skip the multi-line match-context rendering.

### Out / deferred

- §11 history. Wants `widget_event` Up/Down to walk a stored
  history list. Same shape as the branch's implementation —
  preserve `historyIndex` + `historySavedPattern` in `PanelState`,
  intercept Up/Down on `searchField`. Not needed for the scope
  feature; ship after the above six.
- §5 horizontal scroll-to-view long lines. Pure
  `crates/fresh-editor/src/app/navigation.rs` change, unaffected by
  the widget rewrite — the branch's patch reapplies cleanly. Reland
  as a stand-alone commit; not blocked on anything else here.
- §12 bottom-bar polish. Out of scope here.
- §4 mouse expansion (toolbar checkbox click, action-button click,
  per-input-field click). The widget runtime already hit-tests
  everything — verify in tmux instead of writing more code. The
  branch's `mouse_click`-listener machinery is replaced by widget
  hit-areas.

## Suggested PR ordering

Two PRs, both small enough to review in one sitting:

1. **`feat(search-replace): empty-state quality + footer hint + panel passthrough + Alt+A`** (items §17, §15, §18, §10). All four are touched-but-shouldn't-conflict surface area changes (plugin code, i18n strings, one core helper, one keymap line). They land together because none of them block the scope feature and they're all on the path to "this panel reads like a finished tool."

2. **`feat(search-replace): All Files toggle + Search and Replace in Current File`** (item §1, the actual scope feature). Larger, riskier, but cleanly separable. Lands once PR 1 has bedded in so we don't conflate "the scope feature broke" with "the i18n / fallthrough rewrite broke" in any post-merge bisect.

Items §5, §11, §14 stay in `docs/internal/search-replace-ux-improvements.md` as queued follow-ups.

## Implementation notes carried forward

These details from the original branch survive intact and should
inform the redo:

- **Single-file scope filter is a post-filter** on the streaming
  grep callback, not a backend change. Acceptable for v1; the user
  who opened "Search and Replace in Current File" expects fast
  results on a small file, not on a 50k-file repo. If perf becomes a
  problem, add a `pathFilter: string | null` to the
  `grepProjectStreaming` API.
- **Panel title falls back to project mode automatically** when the
  source buffer has no associated path (unnamed buffer, virtual
  scratch, …). Don't surface a broken single-file UI for buffers
  that can't be filtered.
- **`Alt+A` is free in the keymap.** `Alt+R` is taken by
  `resume_live_grep`, `Alt+F` is the File menu, `Alt+S` is the
  Selection menu. `Alt+A` was confirmed free during the branch's
  audit and remains so.
- **Title rename on scope flip needs host help.** The widget runtime
  doesn't currently expose a way to rename a mounted panel's buffer.
  Two options: (a) add `editor.setBufferName(buffer_id, name)` to the
  plugin API (small) — or (b) unmount + remount the panel on scope
  flip and accept the brief flash. The branch did (b) via
  `panel.allFiles !== effectiveAllFiles` triggering a full state
  reset. Pick (a) when the IPC PR lands; until then, (b) is fine.
- **`Replace Selected (N)` is a cosmetic rename**, not a new
  handler. `doReplaceAll` already filters `r.selected`. The button's
  i18n key changes; the action it fires doesn't.

## How to find the original work

The branch `claude/search-replace-improvements-WUcez` is preserved
on origin. Diff
```
git diff 2d57371e8..origin/claude/search-replace-improvements-WUcez \
  -- crates/fresh-editor/plugins/search_replace.ts
```
to see how each section was implemented against the pre-widget
plugin. Most of that diff translates structurally rather than
literally; the per-section "Shape on master" notes above are the
translation table.

A literal `git rebase` would have produced a merge conflict on
nearly every renderer-touching commit. Do not attempt one — the
plan above is the substitute.
