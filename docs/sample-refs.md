# Sample Reference System

This document describes how audio samples are referenced and resolved in the DAW.

## Goals

1. **Explicit semantics** — every sample reference knows *what kind* of path it is,
   not just a raw string that gets guessed at runtime.
2. **Portable projects** — projects can be moved, shared, or archived without
   breaking references.
3. **Dev-friendly** — support a global "dev root" for local development workflows.
4. **Graceful degradation** — missing files don't crash the app; they show as
   "offline" and can be relinked later.

## SampleRef

All audio references in the project file use a `SampleRef` enum rather than raw paths:

```rust
enum SampleRef {
    /// Relative to {dev_root}/samples/
    /// e.g., "cr78/kick-accent.wav" → /Users/korbin/dev/daw/samples/cr78/kick-accent.wav
    DevRoot(PathBuf),

    /// Relative to the project file's directory
    /// e.g., "audio/kick.wav" → {project_dir}/audio/kick.wav
    ProjectRelative(PathBuf),
}
```

### JSON Format

In `.dawproj` files, SampleRef is serialized as a tagged object:

```json
{
  "sample_ref": {
    "kind": "dev_root",
    "path": "cr78/kick-accent.wav"
  }
}
```

Or for project-relative:

```json
{
  "sample_ref": {
    "kind": "project",
    "path": "audio/my-recording.wav"
  }
}
```

## PathContext

Resolution happens through a `PathContext` struct that holds the root directories:

```rust
struct PathContext {
    /// Parent directory of the .dawproj file
    project_root: PathBuf,

    /// Optional dev/workspace root (e.g., /Users/korbin/dev/daw)
    /// DevRoot refs resolve to {dev_root}/samples/{path}
    dev_root: Option<PathBuf>,
}
```

### Resolution Rules

| SampleRef Variant | Resolves To |
|-------------------|-------------|
| `DevRoot("cr78/kick.wav")` | `{dev_root}/samples/cr78/kick.wav` |
| `ProjectRelative("audio/kick.wav")` | `{project_root}/audio/kick.wav` |

If `dev_root` is `None`, `DevRoot` refs cannot be resolved (will be offline).

## Offline Clips

When `PathContext::resolve()` returns `None` (file not found):

1. **Project still loads** — the app does not fail.
2. **Clip is marked offline** — stored with original `SampleRef` and error message.
3. **UI shows warning** — offline clips are visually distinct (grayed out, warning icon).

### Data Model

```rust
pub struct OfflineClip {
    pub track_id: TrackId,
    pub sample_ref: SampleRef,
    pub start_tick: u64,
    pub end_tick: u64,
    pub name: String,
    pub error: String,
}
```

On load, the session logs offline clips:
```
Warning: 3 clip(s) are offline (missing audio files):
  - kick 1 (dev_root:cr78/kick.wav): Sample not found: "cr78/kick.wav"
```

## Default Dev Root

When loading via `Session::from_project(path)`, the dev root defaults to the
grandparent of the project file:

```
/Users/korbin/dev/daw/projects/my_song.dawproj
                    ↑
                 dev_root (grandparent)
```

This means `DevRoot("cr78/kick.wav")` resolves to:
`/Users/korbin/dev/daw/samples/cr78/kick.wav`

For explicit control, use `Session::from_project_with_context(path, Some(dev_root))`.

## Future: Collect Into Project

A planned feature to make projects fully portable:

1. **Scan** all `DevRoot` refs in the project
2. **Copy** referenced files into `{project_dir}/audio/`
3. **Update** refs to `ProjectRelative`

This allows sharing projects without requiring the recipient to have the same
dev root structure.

## Crate Responsibilities

| Crate | Role |
|-------|------|
| `daw_project` | Defines `SampleRef`, `PathContext`, serialization |
| `daw_decode` | Decodes audio from resolved absolute paths |
| `daw_core` | Orchestrates loading, manages `sample_refs` map |

The decode crate has no knowledge of `SampleRef` or path semantics — it only
receives already-resolved absolute paths.

