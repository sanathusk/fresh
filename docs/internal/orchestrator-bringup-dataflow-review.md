# Orchestrator bring-up: data-flow review & simplification notes

Context: issue #2056. Before implementing the root-match fix, this
documents the bring-up data flow end-to-end, the state/branch explosion
it carries, and where a few small data-flow changes would collapse most
of it — so the new design is correct by construction instead of by
careful synchronization.

All claims here are grounded in the code as of this branch and in the
test-backed findings from the #2056 investigation (see the
`orchestrator_bringup_*` tests).

---

## 1. The bring-up pipeline (stages + decision points)

### A. Pre-construction — `main.rs::real_main`
1. Resolve `working_dir`: first non-flag path arg, else `current_dir()`.
2. Load layered config for that dir.
3. Compute flags: `workspace_enabled`, `restore_previous_session`,
   `skip_session_restore_when_files_passed`, `force_restore`,
   `first_run` vs restart.

### B. Editor construction — `editor_init.rs::with_options`
4. `working_dir = canonicalize(working_dir)`. **Never changes again during construction.**
5. `read_persisted_windows_env(data_dir)` → global `windows.json`
   (migrating a legacy per-cwd layout first if present).
6. **Pick** `pick_active_window_for_cwd(env, working_dir)`:
   `env.active` if it matches cwd → else highest-id match → else `None`.
   "matches" = `window_matches_cwd`: **`project_path ?? root == cwd`**.
7. `(active_id, active_root) = picked ?? (WindowId(1), working_dir)`.
8. LSP root + Deno detection keyed off `active_root`.
9. Build the **active** `Window { root: active_root, .. }`; build inert
   **shells** for every other persisted window.
10. `editor.working_dir = working_dir` **(= cwd, NOT `active_root`)**.

### C. Restore — `handle_first_run_setup` (first run) / `restore_editor_workspace` (restart)
11. `restore_full_session = workspace_enabled && !cli_overrides_restore && (force_restore || restore_previous_session)`.
12. If yes: `try_restore_workspace()` (uses `session_name` ? `load_session` : `load(working_dir)`), then `restore_inactive_window_workspaces()`.
13. Else: `try_restore_hot_exit_buffers()`.

### D. Visible UI — `show_file_explorer` → `init_file_explorer`
14. File-explorer root = **`self.working_dir`** (NOT the active window's `root`).
15. Window title project name = **`self.working_dir.file_name()`** (`render.rs`).

### E. "Orchestrator kicks in"
16. The orchestrator is a **plugin**; it renders the dashboard from
    `listWindows()` (the shells from step 9). It does **not** auto-dive
    on startup (verified). `preview_window_id` (editor-global) tracks a
    preview pane. `restore_inactive_window_workspaces` (step 12) eagerly
    rehydrates each shell so previews paint without a dive.

---

## 2. The central problem: `working_dir` is a duplicate of `active_window().root`

`Editor.working_dir` and `Window.root` are **two sources of truth for the
same fact** ("which project is the active window in"). The field's own
doc comment says it is meant to track `self.active_window().root`.

It is kept in sync **manually** at 7 write sites:
- `window_actions.rs:149` (create_window_with_terminal dive)
- `window_actions.rs:178` (rollback)
- `window_actions.rs:279` (`set_active_window`)
- `workspace.rs:2065,2075` and `2112,2125` (the inactive-restore "swap
  working_dir, restore it after" dance)

…plus the construction init (step 10) — **which is the one site that does
NOT sync it to the picked active window's root.** That single omission is
the entire #2056 bug class:

| State at boot (worktree session picked) | value |
|---|---|
| `active_window().root` | worktree |
| `editor.working_dir` | cwd |

Everything downstream that reads `working_dir` (file explorer root,
title, LSP root in some paths, recovery scope, `getCwd()` for plugins)
now disagrees with the active window. Test-verified consequences:
- the file explorer + title root at the cwd while the active window is
  the worktree (the "latent inconsistency"); and
- the moment anything routes through `set_active_window`, `working_dir`
  jumps to the active window's root and the UI re-roots — the visible
  screenshot symptom.

### Recommendation (biggest win)
**Delete the stored `working_dir` field; make it a derived accessor:**

```rust
pub fn working_dir(&self) -> &Path { &self.active_window().root }
```

This:
- makes the invariant `working_dir == active_window().root` **true by
  construction** — the boot inconsistency cannot exist;
- removes all 7 write sites and the entire `restore_inactive_window_
  workspaces` "save/restore working_dir" dance (it only needs to set the
  active-window pointer, or better, take the target root as a parameter);
- removes `set_active_window`'s manual sync line;
- auto-fixes the title (it already derives from `working_dir`).

Caveats to handle:
- A handful of reads happen via `working_dir()` already (it exists as an
  accessor today); the field and accessor must be unified.
- The base window must exist before any `working_dir()` read. In
  `with_options` the active `Window` is built before `EditorParts`, so
  this holds; audit early reads.
- `restore_inactive_window_workspaces` currently flips the global active
  pointer during construction to restore each shell. With a derived
  `working_dir`, prefer passing the shell's `root` explicitly into the
  restore routine instead of mutating the active pointer (avoids a
  transient "wrong active window" window during boot).

---

## 3. The file explorer reads the wrong root (defect #3)

`Window.file_explorer`'s doc says it "rebuilds at `root` on first open",
but `init_file_explorer` (`file_explorer.rs:154-171`) roots the tree at
**`self.working_dir`**. Since the explorer is **per-window**, it should
root at **that window's `root`**.

### Recommendation
Root the explorer at `self.active_window().root` (or make
`init_file_explorer` a `Window` method using `self.root`). Combined with
§2 this is redundant for the active window but still correct for the
intent and robust if the two ever diverge again. It also makes "dive
into window B shows B's tree" automatic (each window's explorer is built
from its own root).

---

## 4. Pick logic carries two notions of identity

`window_matches_cwd` matches on `project_path ?? root`. There are thus
**two** identities for a window: where it *opens* (`root`) and which
project it *belongs to* (`project_path`, orchestrator metadata). Using
`project_path` for the **launch pick** is what lets a worktree session
(root ≠ cwd) be activated by passing the project dir.

### Recommendation
The launch pick should match on **`root` only** ("`fresh <dir>` activates
the window that opens at `<dir>`"). `project_path` stays purely as
orchestrator-dialog grouping metadata. This removes the
`project_path`-vs-`root` branch and the subtle "preferred for
orchestrator sessions" special-case in `window_matches_cwd`.

---

## 5. Clean-base fallback reuses `id 1` → drops a persisted window

When the pick returns `None`, the fallback base is hard-coded to
`WindowId(1)` (step 7). If a persisted window already has `id 1` (e.g.
another project's base), `editor_init`'s shell loop `continue`s on
`id == active_window_id` and that window is **never rebuilt** — it
vanishes from the live map (test-verified in the cross-project branch),
and the next save can overwrite its on-disk record. `editor_init` even
carries a comment explaining the "stale id-1 lending state" hazard it
tries (and fails) to fully avoid.

### Recommendation
Allocate the fallback base a **non-colliding id** (`max(existing)+1` /
`env.next_id`). Removes the shadow/drop and the special-case reasoning.
(Per the design decision, window ids are not user-facing; the
orchestrator shows labels, so a non-1 base id is invisible.)

---

## 6. Restore is implemented twice and drifts

Restore runs through `handle_first_run_setup` (first run) **and**
`restore_editor_workspace` (restart) with overlapping-but-not-identical
logic, plus `restore_inactive_window_workspaces`. The test harness has
its own `startup()` that "mirrors `handle_first_run_setup`" — and we
found it **omits** `restore_inactive_window_workspaces`, i.e. the mirror
already drifted.

### Recommendation
Extract one `fn run_startup_restore(editor, flags)` that both the
first-run and restart paths call, and have the harness call the same
function. One code path = no drift, and the `restore_full_session`
boolean algebra lives in exactly one place.

---

## 7. State-location audit (single-window → multi-window leftovers)

Most per-window state was already migrated onto `Window` (the `Editor`
struct is densely annotated with "moved onto `Window`" markers). The
notable stragglers / observations:

| Field (on `Editor`) | Verdict |
|---|---|
| `working_dir: PathBuf` | **Leftover.** Should be derived from `active_window().root` (§2). The canonical per-window fact is `Window.root`. |
| `last_window_title: Option<String>` | Legit global (one OS terminal title per process), but it is *computed from* `working_dir`; once §2 lands it follows the active window automatically. |
| `terminal_width/height` | Duplicated: also on each `Window` (seeded at build, updated on resize). The editor-level pair is the incoming-size source; windows cache it. Minor — worth a comment clarifying the propagation direction, not a move. |
| `preview_window_id` | Legit global (orchestrator preview pane is a single editor-level concept). |
| `session_name` | Legit global, but **unrelated to orchestrator windows** — it's the *server/attach* session (socket paths, `load_session` vs `load`). Naming collides conceptually with orchestrator "sessions"; consider renaming to `server_session_name` to reduce confusion. |
| `plugin_global_state` | Legit global by design (per-plugin global slot); per-window plugin state lives on `Window.plugin_state`. |

No other obvious "should-be-per-window" fields remain; the migration was
thorough. `working_dir` is the one straggler, and it happens to be the
one causing #2056.

---

## 8. How this simplifies the new design — and status

In order of leverage (✅ = landed on this branch):

1. ✅ **Pick on `root` only** (§4) — `window_matches_cwd` matches the
   window's `root`; removes the worktree-hijack and the
   `project_path`-vs-`root` branch.
2. ✅ **Non-colliding fallback base id** (§5) — a foreign id-1 window is
   re-id'd and preserved as a shell rather than dropped.
3. ✅ **Explorer roots at the active window's `root`** (§3) — and the
   constructor moved onto `Window` (see §3 / Stage 2).
4. ✅ **Derive `working_dir` from `active_window().root`** (§2) — the
   field is deleted; the boot invariant is unconditional; the two
   batch-workspace functions no longer flip a `working_dir` copy.
5. 🟡 **Workspace capture/restore off the active-window flip** (§10) —
   capture moved onto `Window` (done); restore stays editor-coupled
   (opens files via `Editor::open_file`/LSP) and is parameterized by
   `WindowId` to drop the flip (remaining).
6. ⬜ **Unify the restore path** (§6) — first-run vs restart vs harness.

Net effect on the branch/state matrix (✅ already realized):
- ✅ `project_path`-vs-`root` pick branch → gone.
- ✅ `working_dir` vs `active_window().root` divergence → unrepresentable.
- ✅ id-1 fallback collision special-case → gone.
- ✅ explorer "first-init sticky at working_dir" hazard → gone.
- ⬜ active-window-flip dance in workspace save/restore → §10.
- ⬜ two restore implementations → one (§6).

---

## 9. Test coverage (acceptance criteria) — green

`tests/orchestrator_bringup_*` pins the full invariant and passes:
- `*_characterization` — the pick across persistence layouts, incl. the
  converse (`launching_in_a_worktree_restores_that_worktree_session`).
- `*_render_verify` — rendered explorer root / title / dive re-rooting
  (defect #3), and faithful per-window workspace restore incl.
  `external_files` (`launch_restores_the_projects_own_workspace_…`).
- `*_plugin_verify` — same with the orchestrator plugin loaded.

The two gap tests were verified discriminating: temporarily reverting
`window_matches_cwd` to the `project_path` match makes both fail.

---

## 10. Stage 1 plan — workspace capture/restore as a per-window factory

**Problem (recap):** `capture_workspace` / `save_workspace` /
`try_restore_workspace` / `apply_workspace` implicitly act on the active
window (114 `active_window` refs across 18 `restore_*`/`apply_*`
helpers). To save/restore a *non-active* window, the two batch functions
(`save_all_windows_workspaces`, `restore_inactive_window_workspaces`)
temporarily flip the global `active_window` pointer. That flip is a
window-scoped operation faked with editor-global state.

### Finding: capture is window-pure, restore is editor-coupled

Implementation surfaced an asymmetry:

- **Capture (save) is pure window-read.** ✅ Moved:
  `Window::capture_workspace(&self) -> Workspace` reads only the window's
  own state + `self.resources` (no editor reach). `Editor::capture_workspace`
  delegates and injects the editor-global `plugin_global_state`.

- **Restore (apply) is editor-coupled and CANNOT be a pure `Window`
  constructor.** Restoring opens files via
  `open_workspace_files → open_file_internal → Editor::open_file`, which
  pulls in **LSP attach, buffer allocation, grammar/syntax** — all
  editor-level (LSP is editor-spawned, per-window). A
  `Window::from_workspace(...)` constructor would require relocating the
  whole file-open subsystem onto `Window`, which is out of scope and not
  desirable. So **there is no `from_workspace` constructor.**

### Revised restore-side plan: parameterize by `WindowId` (no flip)

`apply_workspace` / `try_restore_workspace` stay on `Editor` but take an
explicit target window instead of implicitly using the active one — that
removes the active-pointer flip without moving the file-open machinery:

```rust
impl Window { pub(crate) fn capture_workspace(&self) -> Workspace; }   // DONE

impl Editor {
    pub fn save_workspace_for(&mut self, id: WindowId) -> Result<(), WorkspaceError>;     // windows[id].capture_workspace() + disk + plugin_global
    pub fn restore_workspace_for(&mut self, id: WindowId) -> Result<bool, WorkspaceError>; // apply into windows[id] via editor services
    // existing names become thin active-window wrappers:
    pub fn save_workspace(&mut self)        -> Result<(), WorkspaceError> { self.save_workspace_for(self.active_window) }
    pub fn try_restore_workspace(&mut self) -> Result<bool, WorkspaceError> { self.restore_workspace_for(self.active_window) }
    // batch ops become plain loops — the active_window flip is deleted:
    pub fn save_all_windows_workspaces(&mut self) -> Result<(), WorkspaceError>;
    pub fn restore_inactive_window_workspaces(&mut self);
}
```

`restore_workspace_for(id)` threads `id` through the ~12 window-scoped
restore helpers (`open_workspace_files`, `restore_split_node`,
`restore_external_files`, `restore_terminals_from_workspace`,
`restore_bookmarks_from_workspace`, `restore_file_explorer_settings`,
`restore_search_options`, `restore_prompt_histories`,
`apply_read_only_flags`, `clean_orphaned_buffers`, `log_restore_summary`,
view-state restore) — changing `self.active_window()/active_window_mut()`
to `self.windows.get(&id)/get_mut(&id)`. The editor-global steps
(`restore_config_overrides`, `plugin_global_state`,
`restore_hot_exit_changes`/`restore_unnamed_buffers` via `recovery_service`,
the `buffer_activated` hook) stay where they are.

### Acceptance
- Round-trip property test (committed: `orchestrator_workspace_roundtrip`)
  stays green — that is the de-risking gate.
- `save_all_windows_workspaces` / `restore_inactive_window_workspaces`
  contain **no** `self.active_window = …` writes.
- All existing `orchestrator_bringup_*` specs stay green.

### Status / why the rest is deferred
Save side (capture → `Window`) is **done**. The restore-side
`WindowId`-parameterization removes the last flip but has **no
correctness payoff** (post-§2 the flip is transient and restored) and
threads an id through ~12 behavior-sensitive helpers. It's safe to do
incrementally now that the round-trip property test guards it; deferred
only by appetite for churn, not by risk of a missing safety net.
