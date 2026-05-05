## MODIFIED Requirements

### Requirement: Reverse geocoding is fail-soft and abstracted via a trait

The system SHALL define a `ReverseGeocoder` interface with a single async method that accepts `(lat, lng)` and returns an optional human-readable region label. The system SHALL ship one implementation backed by Nominatim (or equivalent free service) configured with a fixed argus User-Agent and a 2-second per-request timeout, requesting `zoom=17` and `addressdetails=1`. The implementation SHALL compose the returned label as `"{district} · {road}"` when both an administrative district label (preferring `suburb` > `city` > `town` > `village` > `county` > `state` > `country`) and a `road` field are present in the structured address. When only one is present the label SHALL be that single value. When neither is present the implementation SHALL fall back to Nominatim's free-text `display_name` field, then to `null`. On any failure (timeout, non-2xx response, parse error, network error), the system SHALL store `region_name = null` on the event and SHALL still record the event normally — failure SHALL NOT cause the event-submission request to fail. The handler SHALL invoke the geocoder synchronously as part of event creation.

#### Scenario: Successful geocode with district and road composes both

- **WHEN** an AppUser submits a valid event and the geocoder returns both a district label and a `road` field
- **THEN** the stored event has `region_name` set to `"{district} · {road}"` (e.g., `"信義區 · 忠孝東路五段"`)

#### Scenario: Successful geocode with only district falls back to district

- **WHEN** an AppUser submits a valid event and the geocoder returns a district label but no `road`
- **THEN** the stored event has `region_name` set to the district label alone (e.g., `"信義區"`)

#### Scenario: Successful geocode with only road falls back to road

- **WHEN** an AppUser submits a valid event and the geocoder returns a `road` field but no district
- **THEN** the stored event has `region_name` set to the road alone (e.g., `"忠孝東路五段"`)

#### Scenario: Geocoder timeout produces null region_name

- **WHEN** the geocoder times out (≥ 2 seconds)
- **THEN** the event is still recorded with `region_name = null`
- **AND** the event-submission request returns `201 Created`

#### Scenario: Geocoder error produces null region_name

- **WHEN** the geocoder returns a non-2xx response, malformed payload, or any other error
- **THEN** the event is still recorded with `region_name = null`

#### Scenario: Manual label is preserved across geocoding outcome

- **WHEN** an AppUser submits an event with `manual_label = "公司門口"`
- **THEN** the stored `manual_label` is `"公司門口"` regardless of whether geocoding succeeded
- **AND** the optional `region_name` is set independently

## ADDED Requirements

### Requirement: Reverse geocoding is fronted by an in-memory LRU cache

The system SHALL wrap the production `ReverseGeocoder` in an in-memory caching layer. The cache SHALL key entries on `(lat, lng)` rounded to four decimal places (≈ 11 m grid). The cache SHALL evict entries via LRU policy when its capacity (10,000 entries) is exceeded. Each cached entry SHALL expire after a 1-hour TTL — reads after expiry SHALL be treated as a miss. The cache SHALL be transparent to callers: `lookup(lat, lng)` returns the same value (including `None`) regardless of whether the result came from the upstream geocoder or the cache. The cache SHALL be process-local and SHALL NOT persist across restarts. The test-only `StaticReverseGeocoder` SHALL NOT be cached (tests inject the static geocoder directly so they observe deterministic behavior).

#### Scenario: Repeat lookup within the same grid cell hits the cache

- **WHEN** the system has already resolved `(lat, lng)` to some label within the last hour
- **AND** another lookup arrives for coordinates rounding to the same 4-decimal key
- **THEN** the upstream Nominatim implementation is NOT called
- **AND** the cached label is returned

#### Scenario: Lookup outside the cache window misses

- **WHEN** more than 1 hour has elapsed since the last lookup for a given key
- **THEN** the next lookup with that key triggers an upstream Nominatim call
- **AND** the result replaces the expired entry

#### Scenario: Negative result is cached

- **WHEN** an upstream lookup returns `None` (geocode failure)
- **THEN** subsequent lookups for the same key within the TTL also return `None` without calling upstream

#### Scenario: Cache evicts oldest when full

- **WHEN** the cache holds 10,000 distinct keys and a new key is looked up
- **THEN** the least-recently-used entry is removed before the new entry is stored
