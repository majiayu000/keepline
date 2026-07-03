# LanceDB Vector Dimension And Bulk Maintenance Product Spec

Issue: https://github.com/majiayu000/keepline/issues/99

## Summary

Keepline must keep memory persistence trustworthy when the active embedding
provider changes vector dimensions and when operators run count or session
deletion operations on the LanceDB-backed observation store.

## User Problem

Users can switch between local, OpenAI, and Voyage embedding providers. LanceDB
tables infer vector shape from the first inserted row, so a later provider with
a different dimension can make writes fail. Count and session deletion paths
also should not load the full table into memory.

## Product Behavior

1. Observation writes must reject vector dimensions that do not match the
   configured store dimension before LanceDB mutates data.
2. Existing tables with a different vector dimension must return a clear error
   that names the expected and actual dimensions.
3. Batch writes must reject mixed or incompatible vector dimensions before a
   partial batch is inserted.
4. `count()` must use LanceDB row-count support when available.
5. `deleteBySessionId()` must use a store-level predicate delete path and
   return the deleted count without client-side full-table filtering.
6. Errors must be raised to callers. A warning plus fake success is not allowed.

## Non-Goals

- Do not migrate existing user data automatically in this tranche.
- Do not replace LanceDB or change embedding provider selection.
- Do not change the observation-id injection guards from GH74.

## Acceptance Criteria

1. 384 to 1536 dimension changes are detected with a clear error.
2. 1536 to 384 dimension changes are detected with a clear error.
3. Batch inserts do not partially write incompatible vectors.
4. `count()` does not call `toArray()` on the full table.
5. `deleteBySessionId()` does not load all observations before deleting.
6. Focused tests cover success and failure paths.

