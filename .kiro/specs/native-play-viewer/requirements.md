# Requirements Document

## Introduction

A native Rust app (desktop, Android, iOS) for filtering and animating NFL Big Data Bowl dropback plays with role-based player stats, built as new workspace crates next to the unchanged web app. Details in `plan.md` and `design.md`.

## Glossary

- **Workspace**: The Cargo workspace rooted at the repo, containing the root web app package and the crates `etl`, `nfl_core`, `app`.
- **Web_App**: The existing root package `nfl-analizer` (`src/`, `timeline/`, `nfl.db`).
- **ETL**: The `etl` binary producing `data/nfl.sqlite`.
- **Core**: The GUI-free `nfl_core` library.
- **App**: The egui/eframe `app` crate.
- **Native_DB**: The compact read-only `nfl.sqlite`.
- **Frame_Blob**: The packed per-play tracking data in `play_frames`.
- **Facet**: A dropdown dimension whose options carry label, detail and count.

## Requirements

### Requirement 1: Workspace and preservation

**User Story:** As a developer, I want the native app added without breaking the web app, so that both keep working.

#### Acceptance Criteria

1. THE Workspace SHALL declare members `etl`, `nfl_core`, `app` while keeping the root `[package]`, `[[bin]]`, `[dependencies]` and profiles unchanged.
2. THE Workspace SHALL declare `rusqlite = "=0.32.1"` with feature `bundled` once in `[workspace.dependencies]` and pin every other dependency to an exact version.
3. WHEN any task completes, THE Web_App SHALL build with `cargo build -p nfl-analizer` and serve with `cargo run`.
4. THE Workspace SHALL append `data/*.sqlite` and `app/assets/*.sqlite` to `.gitignore` and add a "Native app" README section while preserving existing content.

### Requirement 2: Teams and style tokens

**User Story:** As a user, I want consistent NFL colors, so that both apps read the same.

#### Acceptance Criteria

1. THE Core SHALL provide a table of 32 unique teams covering every abbreviation in `games.csv`.
2. THE Core SHALL define team primary colors equal to `TEAM_COLORS` in `timeline/viewer.html`.
3. THE Core SHALL define app, field, result and role color tokens as plain RGB constants equal to the values in `plan.md` "Styling".

### Requirement 3: ETL

**User Story:** As a developer, I want a compact DB built from the CSVs, so that it can ship inside the app.

#### Acceptance Criteria

1. WHEN the ETL loads CSVs, THE ETL SHALL store `NA` and the tracking event `None` as NULL.
2. THE ETL SHALL store tracking as one Frame_Blob per play with entity slots in `play_entities`, the ball having NULL nflId.
3. WHEN a Frame_Blob is decoded, THE Core SHALL return positions equal to the source within quantization error and NULL angles as NULL.
4. WHEN a play has `playDirection = left`, THE ETL SHALL normalize x, y and angles so offense moves left to right.
5. THE ETL SHALL store one `play_events` row per distinct event per play, preferring manual tags over auto tags, and compute `play_summary` and `player_play_stats`.
6. WHERE `--offline` is not passed, THE ETL SHALL fill venue, final score and overtime from the nflverse schedule and 128×128 team logos from ESPN.
7. IF an enrichment download fails, THEN THE ETL SHALL print a warning, leave those columns NULL and complete successfully.
8. WHEN the ETL finishes, THE ETL SHALL run `VACUUM`, set `user_version`, atomically replace the output file, print its size and copy it to `app/assets/`.

### Requirement 4: Read-only data access

**User Story:** As a developer, I want a typed read-only DAL, so that the UI never writes data.

#### Acceptance Criteria

1. THE Core SHALL open the Native_DB read-only and return games, teams with logos, plays, players and decoded frames.
2. THE Core SHALL use bound SQL parameters for every query.
3. WHEN a game is at a neutral site, THE Core SHALL report `is_neutral()` as true.

### Requirement 5: Filters and facets

**User Story:** As a user, I want to filter plays with counts in every dropdown, so that I see why combinations are empty.

#### Acceptance Criteria

1. THE Core SHALL filter plays by game, week, team and side, pass result, dropback type, down, quarter, coverage, coverage type, play action and player (via `pff_scouting`).
2. THE Core SHALL return for each Facet an "Any" option first plus options whose count equals the plays matching that option combined with the other active filters.
3. THE Core SHALL sort zero-count options last.
4. THE Core SHALL provide a team → position → player picker without text input.

### Requirement 6: Player stats

**User Story:** As a user, I want role-specific stats, so that I can compare players fairly.

#### Acceptance Criteria

1. THE Core SHALL determine a player's role from the most frequent `pff_role`, falling back to `officialPosition`.
2. THE Core SHALL compute the role-specific stats and weekly trend listed in `plan.md` from SQL aggregates.

### Requirement 7: Geometry and playback

**User Story:** As a user, I want an accurate animated field, so that plays look correct.

#### Acceptance Criteria

1. THE Core SHALL map field coordinates to screen coordinates for landscape and rotated portrait viewports.
2. THE Core SHALL convert an angle θ to the vector (sin θ, cos θ).
3. THE Core SHALL interpolate entity positions between consecutive frames.
4. THE Core SHALL derive phase markers and the dropback span (snap → throw/sack) and clamp the first-down line to the goal line.
5. WHEN a point is tested against players, THE Core SHALL return the closest player within the hit radius.
6. THE Core SHALL choose dark text for team colors with relative luminance above 0.35.

### Requirement 8: App UI

**User Story:** As a user, I want three themed screens on every platform, so that I can browse, watch and inspect.

#### Acceptance Criteria

1. THE App SHALL provide Filters/Play List, Play Viewer and Player Detail screens with back navigation, including Android back.
2. IF the Native_DB is missing or the schema version differs, THEN THE App SHALL show a message screen.
3. WHILE a play is playing, THE App SHALL advance frames at 10 fps times the selected speed (0.5×, 1×, 2×) and request repaints.
4. WHEN a user taps a player, THE App SHALL open that player's detail screen.
5. THE App SHALL resolve the DB path per platform: desktop arg/env/`data/nfl.sqlite`, Android asset copy, iOS bundle.
6. WHEN a game is at a neutral site, THE App SHALL show "vs", no home/away labels and no midfield logo.
7. THE App SHALL build on desktop Linux, Android (APK) and iOS (on macOS).
