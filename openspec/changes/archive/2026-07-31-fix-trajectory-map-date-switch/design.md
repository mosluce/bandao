## Context

`admin-web/pages/checkin/[appUserId]/trajectory.vue` renders a Leaflet map for one AppUser's day. Its template is a four-way `v-if` chain — `loading` → `error` → `!hasData` → map — and the map is created from `watch(hasData)`.

Two facts combine into the reported bug:

1. `loading` flipping to `true` unmounts the `v-else` branch, so the `<div ref="mapContainer">` that Leaflet was initialized against leaves the document. Vue creates a **new** element when the branch remounts; `mapInstance` still points at the old, detached one.
2. `watch(hasData)` only fires on a *change*. Switching from a day with data to another day with data leaves `hasData` at `true` throughout, so `renderMap()` never runs against the new container.

```
切換日期
   │
   ├─▶ loadDay(): loading = true
   │        └─▶ re-render: <div ref=mapContainer> unmounted
   │                       (mapInstance still holds the detached node)
   │
   ├─▶ pings.value / events.value assigned
   │        └─▶ watch([pings, events]) (flush 'pre')
   │              hasData === true && mapInstance !== null → redrawLayers()
   │              …drawn onto the detached map
   │
   └─▶ loading = false
            └─▶ re-render: brand-new empty <div>, nobody calls renderMap() → blank
```

The failure is therefore conditional: `true → false` (day with data → day without) tears the map down and `false → true` re-creates it, so only **day-with-data → day-with-data** goes blank. A reload always takes the `false → true` path, which is why refreshing appears to "fix" it.

The same transition-coupling is already documented in `admin-web/test/pages/trajectory.test.ts:97-108`, where a `defer()` helper delays the mocked fetch specifically so `hasData` flips post-mount. The workaround was adopted instead of removing the coupling.

## Goals / Non-Goals

**Goals:**

- Switching dates renders the new day's map without a reload, for every combination of has-data / no-data / error on either side.
- The Leaflet instance's container can never be orphaned by a Vue re-render.
- Map drawing is a function of the current data, not of a boolean transition — the class of bug disappears rather than being patched at one call site.
- Preserve every existing rendering rule: merged-series ordering and coloring, `admin_force` marker handling, legend, attribution, and fit-bounds on each render.

**Non-Goals:**

- Preserving the user's zoom/pan across date changes. `fitBounds` continues to run on every render (explicitly chosen: the page's job is "show me this day", and a retained viewport that excludes the new day's points would produce a blank-looking map — the very symptom being fixed).
- Adding `invalidateSize()` handling for window resize. Pre-existing gap, unrelated to this bug.
- Revisiting the rule that rendering is keyed off the **merged** point count. A day whose only event is an `admin_force` `clock_out` still counts as zero merged points and shows the empty state even though a marker exists — existing specified behaviour, out of scope here.
- Any API, data-model, or Flutter app change.

## Decisions

### D1: Persistent map container with state overlays

The map container moves out of the `v-if` chain and is always mounted. Loading / error / empty become absolutely-positioned overlays inside the same relative wrapper.

```
<div class="relative rounded-xl border overflow-hidden">
  <div ref="mapContainer" class="h-[600px] w-full" />   ← never unmounted
  <div v-if="loading" class="absolute inset-0 …">載入軌跡中…</div>
  <div v-else-if="error" class="absolute inset-0 …">{{ error }}</div>
  <div v-else-if="!hasData" class="absolute inset-0 …">該日無軌跡資料</div>
  <div v-if="hasData && !error" class="absolute bottom-3 left-3 …">legend</div>
</div>
```

Rationale: this removes the root cause rather than compensating for it. `mapContainer.value` is stable for the page's lifetime, so a Leaflet instance created once stays valid. It also stops the whole card from disappearing and re-appearing on every date change.

Alternatives considered:

- **Compare container identity before redrawing** (`mapInstance.getContainer() !== mapContainer.value → renderMap()`). Smallest diff, but keeps the map tearing down and re-initializing on every date change (tile re-fetch, visible flash) and leaves the fragile "render is triggered by a state transition" shape in place for the next person to trip over.
- **Tear the map down when `loading` goes true, re-create when it goes false.** Correct, but re-creates the map unconditionally on every fetch and still couples rendering to a flag rather than to data.

### D2: Leaflet stays lazily instantiated, and is retained once created

The container is always in the DOM, but `L.map()` is still called only when the page first has a non-empty merged series — preserving the existing requirement that a zero-point day does not initialize Leaflet or fetch tiles.

Once created, the instance is **retained** for the page's lifetime. Moving to a day with zero points clears the drawn layers and shows the overlay instead of calling `teardownMap()`; teardown happens only in `onBeforeUnmount`. Retaining avoids pointless re-initialization when the admin steps across an empty day between two populated ones.

Layers must be cleared on the zero-point and error paths so no fraction of the previous day survives underneath an overlay.

### D3: Rendering is driven by data, replacing both existing watchers

`watch(hasData)` and `watch([pings, events])` collapse into one watcher on the merged series:

```
watch([mergedSeries, events], async () => {
  if (!hasData.value) { clearLayers(); return }
  await nextTick()
  if (!mapInstance) await renderMap()   // creates map + tile layer
  redrawLayers()                        // polylines, markers, fitBounds
})
```

`renderMap()` no longer both creates and draws; it only ensures the instance exists. `redrawLayers()` stays the single drawing path. The watcher runs on *any* data change, so `true → true` transitions are handled identically to `false → true`.

`events` stays in the watch source alongside `mergedSeries` because markers are drawn from the full event list, which includes events the merge contract excludes.

### D4: Loading overlay is translucent; error and empty overlays are opaque

Loading uses a `bg-white/80`-style scrim over whatever was last drawn, labelled 載入軌跡中…. The previous day's path is dimmed but not fully hidden, which avoids a hard flash on every date change and reads as "refreshing". Because it is labelled and short-lived it will not be mistaken for the new day's data.

Error and empty overlays are opaque: in both cases the layers have already been cleared (D2), so there is nothing behind them, and an opaque surface keeps 該日無軌跡資料 legible against tiles.

### D5: Legend is gated on having data

The legend currently lives inside the map's `v-else` branch and so implicitly hides in every other state. With a persistent container it needs an explicit `v-if="hasData && !error"`, otherwise the reported screenshot's exact look — a legend floating over an empty white rectangle — becomes the *designed* empty state.

Overlays sit above Leaflet's panes and the legend (`z-[1010]`, versus the legend's `z-[1000]`) and below the export modal (`z-[1100]`).

### D6: Test assertions move from container presence to Leaflet instantiation

`[data-testid="trajectory-map"]` is now always present, so the existing empty-state assertion `find('[data-testid="trajectory-map"]').exists() === false` no longer describes anything. It is replaced by asserting `L.map` was not called plus the presence of `[data-testid="trajectory-empty"]`. A new regression test drives a second `loadDay()` with different data and asserts the second day's geometry reaches Leaflet.

## Risks / Trade-offs

- **The `defer()` mount-timing workaround in the existing test suite may mask a regression.** → Once rendering is data-driven, a synchronously-resolving mock should also produce a map. Keep `defer()` for the existing cases (changing it is not required here) but write the new date-switch test against a mount that has already settled, so it exercises the `true → true` path the old code could not handle.

- **Leaflet initialized against a container that is mounted but conceptually "not showing".** → The container has a fixed `h-[600px] w-full` and is never hidden with `display:none`, so `L.map()` always measures a real box. If a future change puts the map behind a collapsed section, `invalidateSize()` will be required — noted, not handled here.

- **Retaining the map across a zero-point day means tiles for a stale viewport stay loaded behind an opaque overlay.** → Negligible; the overlay is opaque and `fitBounds` re-frames on the next populated day.

- **Overlay z-index vs. Leaflet's internal panes.** → Leaflet's control pane sits at `z-index: 800` and popups at `700`; `z-[1010]` clears both, and the existing modal at `z-[1100]` still wins. A popup left open when the overlay appears is covered, which is the desired outcome.

- **Behaviour change visible to admins: the card no longer disappears while loading.** → Intentional and an improvement, but it is a visual difference worth mentioning in the change's manual verification.
