# Implementation Plan: Native Play Viewer

## Overview

Follows plan.md's 14 tasks in Rust. Every task ends with `cargo build -p nfl-analizer` and `cargo test --workspace` green; never edit `src/`, `timeline/`, `docs/` or root package deps.

## Tasks

- [ ] 1. Workspace scaffold, teams, style
  - [ ] 1.1 Add `[workspace]` and `[workspace.dependencies]` to root Cargo.toml; create `etl`, `nfl_core`, `app` crates; append to `.gitignore`
    - Pin `rusqlite =0.32.1` bundled, `csv`, `serde`, `ureq`, `eframe`/`egui`/`egui_plot`/`egui_extras`, `image`, `lz4_flex`, `proptest` exactly
    - _Requirements: 1.1, 1.2, 1.4_
  - [ ] 1.2 Implement `nfl_core::teams` (32 teams) and `nfl_core::style` tokens copied from `viewer.html`
    - _Requirements: 2.1, 2.2, 2.3_
  - [ ] 1.3 Themed placeholder screen in `app` showing glyph samples
    - _Requirements: 2.3_
  - [ ]* 1.4 Unit tests: 32 unique teams cover `games.csv`; primaries equal parsed `TEAM_COLORS`
    - _Requirements: 2.1, 2.2_

- [ ] 2. Platform spike
  - [ ] 2.1 Desktop placeholder opens a tiny bundled SQLite and shows a row count
    - _Requirements: 8.5, 8.7_
  - [ ] 2.2 Ask the user before installing Android SDK/NDK, patched `cargo-apk` and Rust Android targets; install only with consent
    - _Requirements: 8.7_
  - [ ] 2.3 Add `android_main` entry, `android-native-activity` feature, asset DB copy; build APK
    - _Requirements: 8.5, 8.7_
  - [ ]* 2.4 iOS simulator build on macOS (manual, Xcode project)
    - _Requirements: 8.7_
  - [ ] 2.5 Document setup steps in a new README "Native app" section
    - _Requirements: 1.4_

- [ ] 3. Schema
  - [ ] 3.1 `nfl_core::schema` DDL, indexes, `create_all`, `SCHEMA_VERSION`
    - _Requirements: 3.2, 3.5, 3.8_
  - [ ]* 3.2 Unit test: every table, column, index present in in-memory DB
    - _Requirements: 3.2_

- [ ] 4. ETL flat CSVs and enrichment
  - [ ] 4.1 Port `cell()` and import loop into `etl::csv_load`; temp file + rename in `etl::main`
    - _Requirements: 3.1, 3.8_
  - [ ] 4.2 Port schedule fill (add scores, overtime) and ESPN logos (LA→lar, WAS→wsh, 128×128); `--offline`; warn on failure
    - _Requirements: 3.6, 3.7_
  - [ ]* 4.3 Fixture tests: row counts, NA→NULL, neutral game with stadium/score, `--offline` leaves NULLs
    - _Requirements: 3.1, 3.6, 3.7_

- [ ] 5. ETL tracking
  - [ ] 5.1 `nfl_core::blob` encode/decode and `geometry::normalize`
    - _Requirements: 3.2, 3.3, 3.4_
  - [ ]* 5.2 Property test: blob round-trip
    - **Property 1: Blob round-trip**
    - **Validates: Requirements 3.2, 3.3**
  - [ ]* 5.3 Property test: normalization involution
    - **Property 2: Normalization is an involution**
    - **Validates: Requirements 3.4**
  - [ ] 5.4 `etl::tracking`: entities, blobs, event dedup, summary, player_play_stats, VACUUM, size report, copy to `app/assets/`
    - _Requirements: 3.2, 3.5, 3.8_
  - [ ]* 5.5 Fixture tests: frame counts, ball NULL nflId, left play normalized, dedup, snap/throw frames
    - _Requirements: 3.2, 3.4, 3.5_

- [ ] 6. Core models and DAL
  - [ ] 6.1 Structs and read-only `Db` (games, teams, plays, player, frames, `is_neutral`)
    - _Requirements: 4.1, 4.2, 4.3_
  - [ ]* 6.2 Unit tests on seeded DB, including neutral game
    - _Requirements: 4.1, 4.3_

- [ ] 7. Filters and facets
  - [ ] 7.1 `PlayFilter`, `query_plays`, generic facet builder with labels/details, player picker
    - _Requirements: 5.1, 5.2, 5.3, 5.4_
  - [ ]* 7.2 Property test: facet counts sum to Any
    - **Property 4: Facet counts sum to Any**
    - **Validates: Requirements 5.2, 5.3**
  - [ ]* 7.3 Unit tests: each dimension, combinations, labels
    - _Requirements: 5.1_

- [ ] 8. Player stats
  - [ ] 8.1 `player_role`, `player_stats`, weekly trend
    - _Requirements: 6.1, 6.2_
  - [ ]* 8.2 Unit tests: QB, blocker, rusher roles and stats
    - _Requirements: 6.1, 6.2_

- [ ] 9. Geometry and animation helpers
  - [ ] 9.1 `field_to_screen`, `angle_vec`, `lerp_frame`, markers, first-down clamp, `hit_test`, `is_light`
    - _Requirements: 7.1–7.6_
  - [ ]* 9.2 Property test: angle → vector
    - **Property 3: Angle to unit vector**
    - **Validates: Requirements 7.2**
  - [ ]* 9.3 Property test: interpolation bounds
    - **Property 5: Interpolation bounds**
    - **Validates: Requirements 7.3**
  - [ ]* 9.4 Property test: hit-testing
    - **Property 6: Hit-test returns nearest within radius**
    - **Validates: Requirements 7.5**
  - [ ]* 9.5 Property test: field mapping corners
    - **Property 7: Field mapping corners**
    - **Validates: Requirements 7.1**
  - [ ]* 9.6 Unit tests: markers, goal clamp, NO/PIT get dark text
    - _Requirements: 7.4, 7.6_

- [ ] 10. Checkpoint - Ensure all tests pass, ask the user if questions arise.

- [ ] 11. App shell and navigation (plan task 10)
  - [ ] 11.1 `AppState`, `Screen`, back stack, Android back, safe areas, `db_path`, DB-missing screen, visuals from style, logo textures
    - _Requirements: 8.1, 8.2, 8.5_
  - [ ]* 11.2 State tests: default Filters, back works, missing DB message
    - _Requirements: 8.1, 8.2_

- [ ] 12. Filters and play list screen (plan task 11)
  - [ ] 12.1 Pickers with faceted rows, virtualized list, tap → viewer
    - _Requirements: 5.2, 5.4, 8.1_
  - [ ]* 12.2 State test: filter updates list and other facets
    - _Requirements: 5.2_

- [ ] 13. Animated play viewer (plan task 12)
  - [ ] 13.1 Field, players, ball, header, chips, timeline (play/pause, scrub, speed, markers)
    - _Requirements: 7.4, 8.3, 8.6_
  - [ ]* 13.2 State tests: playback advance, neutral game shows no midfield logo
    - _Requirements: 8.3, 8.6_

- [ ] 14. Player detail (plan task 13)
  - [ ] 14.1 Tap-through and role charts with `egui_plot`
    - _Requirements: 6.2, 8.4_
  - [ ]* 14.2 State test: tap maps to nflId and loads stats
    - _Requirements: 8.4_

- [ ] 15. Polish and release (plan task 14)
  - [ ] 15.1 Extend README "Native app" flow; desktop release build; release APK (if toolchain installed)
    - _Requirements: 1.4, 8.7_
  - [ ]* 15.2 iOS release build on macOS (manual)
    - _Requirements: 8.7_

- [ ] 16. Final checkpoint - Ensure all tests pass, ask the user if questions arise.

## Notes

- `*` tasks are optional; 2.2 requires asking the user first.
- Web app must stay green after every task.

## Task Dependency Graph

```json
{
  "waves": [
    { "id": 0, "tasks": ["1.1"] },
    { "id": 1, "tasks": ["1.2", "2.1"] },
    { "id": 2, "tasks": ["1.3", "1.4", "2.2", "3.1"] },
    { "id": 3, "tasks": ["2.3", "3.2", "4.1", "5.1"] },
    { "id": 4, "tasks": ["2.4", "4.2", "5.2", "5.3"] },
    { "id": 5, "tasks": ["2.5", "4.3", "5.4"] },
    { "id": 6, "tasks": ["5.5", "6.1"] },
    { "id": 7, "tasks": ["6.2", "7.1", "8.1", "9.1"] },
    { "id": 8, "tasks": ["7.2", "7.3", "8.2", "9.2", "9.3", "9.4", "9.5", "9.6"] },
    { "id": 9, "tasks": ["11.1"] },
    { "id": 10, "tasks": ["11.2", "12.1"] },
    { "id": 11, "tasks": ["12.2", "13.1"] },
    { "id": 12, "tasks": ["13.2", "14.1"] },
    { "id": 13, "tasks": ["14.2", "15.1"] },
    { "id": 14, "tasks": ["15.2"] }
  ]
}
```
