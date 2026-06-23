# Project Workspaces Product Spec

## Summary

Project Workspaces makes project identity a first-class way to monitor agent work. A user with many Warp panes and local agent runtime sessions should be able to open Keepline and immediately see which projects are active, which runtime adapter is represented in each project, and what needs attention.

This extends the existing Projects tab from a directory card view into a reliable project workspace surface. It is inspired by stash's project-first workboard, but Keepline remains a session monitor and recovery dashboard, not a todo system.

## User Problem

Power users often run several agent sessions at once across multiple repos, worktrees, and terminal tabs. Terminal labels such as Warp tab titles are useful visual hints, but they are not reliable state. The product needs a durable project grouping that comes from the session working directory and repository root, then lets the user filter, inspect, and recover sessions by project.

## Behavior

1. Project identity is based on a normalized project root, not only the last path segment. Two paths with the same basename, such as two worktrees named `keepline`, must appear as separate projects when their roots differ.

2. When a session cwd is inside a git repository, the project is the repository or worktree root. When git metadata is unavailable, the project falls back to the session cwd. When cwd is missing or unreadable, the session appears under an `Unknown project` group instead of disappearing.

3. The Projects view shows one project card per project root. Each card shows project name, disambiguating path, status counts, latest active task, last activity time, total sessions, and token or cost totals when usage data is available.

4. Project cards distinguish agent clients. If both Claude Code and Codex sessions exist for one project, the card shows client badges and per-client counts. A client parser failure must not hide sessions from another client.

5. Selecting a project filters the Sessions view by exact project root. This filter is separate from text search, so a project path must not accidentally match unrelated titles, prompts, files, or similarly named directories.

6. Project filters compose with existing status, search, client, sorting, and pagination controls. Clearing the selected project returns the Sessions view to the previous global filter state.

7. The Projects view always uses the full unfiltered session set for aggregation. Search or status filters in the Sessions tab must not hide projects from the Projects overview.

8. Real-time updates keep project cards current. When a session starts, stops, changes status, updates activity, or changes its visible current task, the owning project's counts, last activity, and task snippet update without a manual refresh.

9. Notifications remain session-based but include project context when available, such as `keepline: waiting` or `mutil-om: lost`. Notification delivery must not depend on the current project filter.

10. Empty, loading, and error states are explicit. No sessions shows an empty Projects view. Project root resolution errors show a degraded grouping by cwd and expose an error indicator in logs or diagnostics, not a blank UI.

11. The UI is keyboard reachable. A user can tab to a project card, press Enter to filter to that project, and return to all sessions without using a mouse. Focus should move predictably after switching tabs.

12. Absolute paths are useful for disambiguation but should be visually compact. Cards may elide home-directory prefixes, while project detail or tooltip surfaces can show the full path.

## MVP Scope

- Derive project groups from existing session directories.
- Add stable project identity and exact project filtering.
- Show client badges where client data exists.
- Keep current session recovery, status, usage, and notification flows working under project filters.
- Add tests that prove same-basename paths do not merge.

## Non-Goals

- No direct Warp tab inspection or Warp tab renaming. Warp is a shell surface, not the source of project truth.
- No todo capture, project knowledge, milestones, decisions, or lessons in MVP.
- No cloud sync or cross-machine project merge.
- No automatic repo mutation or agent spawning from the project card.
- No large visual redesign beyond the project grouping and filtering workflow.

## Open Questions

- Should users be able to assign custom project aliases and icons in the first implementation, or should aliases wait until project metadata is persisted?
- Should archived or completed-only projects remain visible forever, or should the UI default to a recent-activity window with an explicit "show older" control?
- Should project identity use the raw worktree root, the canonical git common directory, or both when multiple worktrees point at the same repository?
