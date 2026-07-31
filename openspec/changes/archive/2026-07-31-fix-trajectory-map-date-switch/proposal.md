## Why

Switching the date on the admin trajectory page leaves the map area blank; only a full page reload brings it back. The page's Leaflet instance is bound to a DOM node that the `v-if` chain unmounts the moment `loading` flips true, and the render trigger is a `watch(hasData)` that fires only on a *change* — so switching between two days that both have data never re-creates the map in the fresh container. Admins lose the page's primary function after one interaction.

## What Changes

- The map container element becomes **persistent** for the page's lifetime instead of living inside the `v-if` / `v-else-if` chain. Leaflet is still instantiated lazily on first data, but once created it is never orphaned by a re-render.
- Loading, error, and empty states become **overlays** positioned over the map container rather than branches that replace it.
- Map drawing is driven by the **current data**, not by a `hasData` boolean transition: whenever the merged series changes and is non-empty, the page ensures the map exists and redraws.
- When the merged point count drops to `0` (or the fetch errors), previously drawn layers are cleared so no stale day is left underneath the overlay.
- Existing behaviour preserved: fit-bounds still runs on every render, so each date change re-frames the viewport to that day's points.

No API, data-model, or app-side change.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `admin-trajectory-dashboard`: the rendering rules for the trajectory page change from "merged point count selects which of several mutually exclusive blocks is mounted" to "a persistent map container with state overlays". The `0` / `1` / `>= 2` behaviour table, the "zero points hides the map" scenario, and the requirement that a date change reruns the fetch all need restating, plus new scenarios covering date-to-date transitions.

## Impact

- `admin-web/pages/checkin/[appUserId]/trajectory.vue` — template restructure (persistent container + overlays) and watcher rework.
- `admin-web/test/pages/trajectory.test.ts` — assertions keyed on `[data-testid="trajectory-map"]` presence/absence no longer describe the empty state; they move to "was Leaflet instantiated" plus overlay presence. The `defer()` helper's workaround comment (which documents this exact transition-coupled render bug) can be revisited once rendering no longer depends on a boolean flip.
- No changes to `api/`, `app/`, or `admin-web/utils/trajectoryMerge.ts`.
