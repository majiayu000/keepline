# Shared Session Status Presentation Product Spec

Issue: https://github.com/majiayu000/keepline/issues/103

## Summary

Session status labels, icons, and grouping semantics should not drift between
the Web UI and Ink terminal UI.

## Product Behavior

1. Keepline must define one semantic status presentation contract for all
   `SessionStatus` values.
2. Web and Ink surfaces must consume the same status vocabulary.
3. Per-theme visual differences are allowed only for color or icon style, not
   for status meaning.
4. Tests must fail when a new `SessionStatus` lacks Web or Ink presentation.

## Non-Goals

- Do not redesign UI layout.
- Do not remove theme-specific colors.
- Do not add new session states.

## Acceptance Criteria

1. All five statuses have shared labels and ordering.
2. Web filters and cards use the shared labels.
3. Ink dashboard views use the same semantic labels or documented aliases.
4. Tests cover both Web and Ink status coverage.

