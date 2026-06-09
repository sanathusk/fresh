# Per-session backends, trust, and env — final design

> Status: **design target**, partially landed. Realizes the
> [`AUTHORITY_DESIGN.md`](AUTHORITY_DESIGN.md) §"Evolution: per-session
> authority" direction and closes the gaps tracked in issue #2280.
> Keep this short; deep mechanics belong in `AUTHORITY_DESIGN.md` /
> `K8S_AUTHORITY_DESIGN.md`.

## Problem

A session (project / window) should fully own *where it runs* (local /
container / SSH / Kubernetes), *whether it's trusted*, and *which dev
environment it has activated*. Today the live `Authority`, `WorkspaceTrust`,
and `EnvProvider` are effectively process-wide: one is fanned across every
window at boot/restart. Visible consequences (issue #2280): remote sessions
come back **local** after a restart/relaunch, and trusting/activating one
project bleeds into others.

Already landed: an installed backend no longer leaks onto *other* windows
when you switch (each window owns `resources.authority`; background windows
are built local instead of inheriting the active backend). The rest of this
doc is the remaining design.

## Core model: a session owns a *Session Profile*

Give every session a small, declarative **`SessionProfile`** — the data
needed to *rebuild* its world — alongside its live handles:

```
SessionProfile {
    backend: BackendSpec,   // Local | Plugin(AuthorityPayload) | RemoteAgent(RemoteAgentSpec)
    trust:   TrustDecision, // this session's level (+ key into a shared registry)
    env:     EnvSpec,       // activated venv/direnv/mise recipe, or none
}
```

`BackendSpec` reuses the existing `AuthorityPayload` / `RemoteAgentSpec`
verbatim, so there is no new backend vocabulary and core stays
backend-opaque (`AUTHORITY_DESIGN.md` principle 3). The profile is set
wherever a backend/trust/env is installed and is the *source of truth* for
restoration; the live `Authority` is derived from it.

### Restoring agent terminals: the *restore command*

Bringing a session's backend back is not enough for an **agent** session
(`claude`, `aider`, …): its seed terminal ran a process that is gone, and
re-opening a bare shell loses the agent. So each terminal carries an
optional **restore command** — the argv to re-run on restore — which is
deliberately **not** the launch argv:

- The **launch command** is what spawned the PTY (often just a shell, or
  `claude`).
- The **restore command** is *how to bring this terminal back*, and it is
  **mutable**: set at create time (defaulting to the launch argv) and
  **updated while the terminal runs** — e.g. once the agent knows its
  session id it sets `claude --resume <id>`. A plugin op
  (`setTerminalRestoreCommand(terminalId, argv)`) writes it; the orchestrator
  / agent plugin owns the policy.

Persisted per terminal (in the workspace file's terminal entries, alongside
cwd/scrollback). On restore the terminal resumes under the existing
focus-to-resume model (it is *not* auto-re-executed in the background): when
the user focuses a restored terminal that has a restore command, it spawns
that command instead of the default shell. A terminal with no restore
command behaves as today (shell + read-only scrollback). For remote sessions
the existing `RemoteAgentSpec.command` is just the seed terminal's initial
restore command.

## Lifecycle: Live vs Dormant

Each session's authority is in one of two states:

- **Live** — connection established (local always; remote after a successful
  connect). Routes every primitive.
- **Dormant** — profile known, not connected. The window runs on a **local
  placeholder** authority (instantly usable, never holds a dead remote
  handle) but is *presented as its real backend, disconnected* — reusing the
  existing `RemoteIndicatorState::Disconnected`/`Connecting` facet. Reads /
  terminals that require the real backend are gated until it activates.

> Only one authority is the active router at a time (principle 2 intact);
> background **live** sessions keep their connection warm via their own
> `session_keepalive`, exactly as today.

## Persistence

The profile round-trips through the **per-dir workspace file** (the session
registry — there is no central `windows.json` for sessions anymore). Saved
on the same paths that already persist a session (`save_all_windows_workspaces`,
pre-restart, pre-quit); read back by session discovery at construction. A
missing profile reads as `Local` (back-compat).

## Restore

Construction (cold launch **and** the `install_authority` restart, which
both rebuild from disk) builds each session's authority **from its profile**:

- `Local` → local authority.
- Remote / container → **Dormant** (placeholder + retained profile).
- The **active** session, if remote, is queued to reconnect immediately
  (surface `Connecting → Connected / FailedAttach`); background sessions stay
  dormant until used.

This replaces today's "fan one authority onto every window."

## Reconnect (on switch or explicit)

Activating a dormant remote session reconnects **that session only** — the
per-window activation `AUTHORITY_DESIGN.md` calls for:

- SSH / Kubernetes → reuse `connect_ssh_authority` / `connect_kube_authority`
  (async, via the existing `RemoteAttachReady` bridge), then
  `set_session_authority(id, authority)` and park the keepalive in
  `session_keepalives[id]`.
- Container → core can't run `devcontainer up`; fire a
  `session_reattach_requested { window_id, profile }` hook so the
  devcontainer plugin re-attaches. Core stays opaque.

Reconnect is **trust-gated** (below). A dead container/pod surfaces
`FailedAttach`, not a crash.

## Per-session trust and env

`WorkspaceTrust` and `EnvProvider` move from one shared handle to one **per
session**, carried in the `SessionProfile` and constructed per window:

- **Trust** — each session has its own level; a small shared **registry**
  (`remember this host/cluster`) lets a decision be reused without making it
  global. Auto-reconnect on restore consults the session's trust (don't
  silently re-establish a remote backend for an untrusted folder).
- **Env** — each session restores its own activation; activating in one never
  affects another.

Switching sessions therefore never changes another session's backend, trust,
or env.

## Invariants

1. One **active** authority routes everything; background sessions are live
   (warm) or dormant, routing nothing.
2. Core never names a backend — profiles carry opaque payloads; the
   Orchestrator renders the "remote facet" generically.
3. The live `Authority` is always derivable from the `SessionProfile`; the
   profile, not the live handle, is what persists.

## Trade-offs

- **Reuses existing payload + connect + indicator machinery** → additive,
  back-compat. Cost: `AuthorityPayload` / `RemoteAgentSpec` now double as a
  persistence format and must stay serde-stable.
- **Connect only the active session; reconnect background lazily on switch**
  → bounds startup cost, avoids N hanging connects (matches the warm/cold
  split in `K8S_WORKSPACE_UX_DESIGN.md`). Cost: a switch into a cold remote
  session has connect latency (shown via the spinner).
- **Container restore needs the plugin** (only it runs `devcontainer up`), so
  core hands off via a hook. Cost: a small plugin contract.
- **Per-session trust** needs a trusted-host/cluster registry to stay usable;
  without it, every restored remote session re-prompts.

## Phasing (each step independently testable)

1. `SessionProfile.backend` + per-window field + workspace-file persistence;
   spec-driven **Dormant** restore (no reconnect yet). Fixes "comes back
   local" → "comes back disconnected, profile retained."
2. Reconnect-on-activate for SSH / Kubernetes; container reattach hook.
3. Per-session `WorkspaceTrust` + trusted-host registry; trust-gate reconnect.
4. Per-session `EnvProvider`.
5. Warm background remote sessions (per-session keepalives surviving restart).
