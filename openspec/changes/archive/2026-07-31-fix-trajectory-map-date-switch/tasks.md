## 1. Template restructure

- [x] 1.1 Move `<div ref="mapContainer">` out of the `v-if` chain into a permanently-mounted `relative` wrapper card, keeping `h-[600px] w-full` and `data-testid="trajectory-map"`
- [x] 1.2 Convert the loading state into an absolutely-positioned overlay (`absolute inset-0`, translucent scrim, `載入軌跡中...`) at `z-[1010]`
- [x] 1.3 Convert the error state into an opaque `absolute inset-0` overlay at `z-[1010]`, preserving the existing red styling and `error` message text
- [x] 1.4 Convert the empty state into an opaque `absolute inset-0` overlay at `z-[1010]`, keeping `data-testid="trajectory-empty"`, the `該日無軌跡資料` copy, and the `換日期` button wired to `focusDatePicker()`
- [x] 1.5 Gate the legend on `hasData && !error` so it never renders over an empty or errored surface; confirm it stays below the overlays (`z-[1000]`) and the export modal stays above (`z-[1100]`)

## 2. Render lifecycle rework

- [x] 2.1 Split `renderMap()` so it only ensures the Leaflet instance and tile layer exist for `mapContainer.value`, and no longer calls `redrawLayers()` itself
- [x] 2.2 Add a `clearLayers()` helper that removes every non-`TileLayer` layer, and reuse it from `redrawLayers()` instead of the inline `eachLayer` block
- [x] 2.3 Replace `watch(hasData)` and `watch([pings, events])` with a single watcher on `[mergedSeries, events]` that clears layers when the merged count is `0`, and otherwise awaits `nextTick()`, ensures the map exists, then redraws
- [x] 2.4 Stop tearing the map down when a day has zero merged points — retain the instance and keep `teardownMap()` on `onBeforeUnmount` only
- [x] 2.5 Clear drawn layers on the error path in `loadDay()`'s `catch` (the existing `pings = []` / `events = []` already drives this through the watcher — verify rather than duplicate)
- [x] 2.6 Confirm `fitBounds` still runs on every redraw so each date change re-frames the viewport

## 3. Tests

- [x] 3.1 Update the empty-state test: `[data-testid="trajectory-map"]` is now always present, so assert `leafletModule.map` was not called plus `[data-testid="trajectory-empty"]` exists
- [x] 3.2 Update the has-data tests that asserted `trajectory-empty` absence / `trajectory-map` presence so they assert on Leaflet instantiation and drawn geometry instead of container presence
- [x] 3.3 Add a regression test: mount with day A's data settled, then change `dateInput` to day B with different mocked data, and assert the polyline/marker calls carry day B's coordinates
- [x] 3.4 Add a test for day-with-data → day-with-zero-points → day-with-data, asserting the empty overlay appears in the middle step and the third day's geometry is drawn
- [x] 3.5 Add a test that a failed fetch shows the error overlay and a subsequent successful date renders that day's geometry

## 4. Verification

- [x] 4.1 Run `npm run test` in `admin-web` (57 passed; no `lint` script is configured in `admin-web/package.json`)
- [x] 4.2 Run `nuxt typecheck` for `admin-web`
- [x] 4.3 Manually verify in the browser against a real Org: switch between two dates that both have trajectory data and confirm the map redraws without a reload
- [x] 4.4 Manually verify the loading overlay does not make the card disappear, and that the legend is hidden on empty and error states
