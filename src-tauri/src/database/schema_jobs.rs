use crate::error::{CommandError, CommandResult};
use rusqlite::Transaction;

pub(crate) fn migrate_v7(transaction: &Transaction<'_>) -> CommandResult<()> {
    transaction
        .execute_batch(
            r#"
        CREATE TABLE background_jobs (
          id TEXT PRIMARY KEY,
          project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
          worktree_id TEXT REFERENCES worktrees(id) ON DELETE SET NULL,
          kind TEXT NOT NULL,
          target_type TEXT,
          target_id TEXT,
          status TEXT NOT NULL CHECK(status IN ('queued','running','analyzing','completed','failed','cancelled')),
          progress REAL NOT NULL DEFAULT 0.0 CHECK(progress >= 0.0 AND progress <= 1.0),
          stage TEXT NOT NULL DEFAULT 'Queued',
          error_message TEXT,
          cancel_requested INTEGER NOT NULL DEFAULT 0,
          result_path TEXT,
          metadata_json TEXT NOT NULL DEFAULT '{}',
          created_at TEXT NOT NULL,
          started_at TEXT,
          completed_at TEXT,
          updated_at TEXT NOT NULL
        );
        CREATE INDEX idx_background_jobs_project
          ON background_jobs(project_id, created_at DESC);
        CREATE INDEX idx_background_jobs_status
          ON background_jobs(status, updated_at DESC);

        CREATE TABLE sprite_sheets (
          id TEXT PRIMARY KEY,
          project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
          worktree_id TEXT REFERENCES worktrees(id) ON DELETE SET NULL,
          animation_id TEXT NOT NULL REFERENCES animations(id) ON DELETE CASCADE,
          job_id TEXT REFERENCES background_jobs(id) ON DELETE SET NULL,
          name TEXT NOT NULL,
          layout TEXT NOT NULL CHECK(layout IN ('horizontal','vertical','grid')),
          frame_width INTEGER NOT NULL CHECK(frame_width > 0),
          frame_height INTEGER NOT NULL CHECK(frame_height > 0),
          padding INTEGER NOT NULL DEFAULT 0 CHECK(padding >= 0),
          spacing INTEGER NOT NULL DEFAULT 0 CHECK(spacing >= 0),
          rows INTEGER NOT NULL CHECK(rows > 0),
          columns INTEGER NOT NULL CHECK(columns > 0),
          scale INTEGER NOT NULL DEFAULT 1 CHECK(scale BETWEEN 1 AND 8),
          transparent INTEGER NOT NULL DEFAULT 1,
          alignment TEXT NOT NULL DEFAULT 'bottom_center' CHECK(alignment IN ('top_left','center','bottom_center')),
          pivot_x REAL NOT NULL DEFAULT 0.5,
          pivot_y REAL NOT NULL DEFAULT 1.0,
          png_path TEXT NOT NULL,
          metadata_path TEXT NOT NULL,
          width INTEGER NOT NULL CHECK(width > 0),
          height INTEGER NOT NULL CHECK(height > 0),
          frame_count INTEGER NOT NULL CHECK(frame_count > 0),
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );
        CREATE INDEX idx_sprite_sheets_project
          ON sprite_sheets(project_id, updated_at DESC);
        CREATE INDEX idx_sprite_sheets_animation
          ON sprite_sheets(animation_id, updated_at DESC);

        CREATE TABLE sprite_sheet_items (
          id TEXT PRIMARY KEY,
          sprite_sheet_id TEXT NOT NULL REFERENCES sprite_sheets(id) ON DELETE CASCADE,
          animation_id TEXT NOT NULL REFERENCES animations(id) ON DELETE CASCADE,
          asset_id TEXT NOT NULL REFERENCES assets(id) ON DELETE RESTRICT,
          position INTEGER NOT NULL CHECK(position >= 0),
          row_index INTEGER NOT NULL CHECK(row_index >= 0),
          column_index INTEGER NOT NULL CHECK(column_index >= 0),
          x INTEGER NOT NULL CHECK(x >= 0),
          y INTEGER NOT NULL CHECK(y >= 0),
          width INTEGER NOT NULL CHECK(width > 0),
          height INTEGER NOT NULL CHECK(height > 0),
          duration_ms INTEGER NOT NULL CHECK(duration_ms > 0),
          UNIQUE(sprite_sheet_id, position)
        );
        CREATE INDEX idx_sprite_sheet_items_sheet
          ON sprite_sheet_items(sprite_sheet_id, position);

        CREATE TABLE vfx_effects (
          id TEXT PRIMARY KEY,
          project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
          worktree_id TEXT NOT NULL REFERENCES worktrees(id) ON DELETE CASCADE,
          animation_id TEXT REFERENCES animations(id) ON DELETE SET NULL,
          name TEXT NOT NULL,
          effect_type TEXT NOT NULL,
          blend_mode TEXT NOT NULL DEFAULT 'normal' CHECK(blend_mode IN ('normal','add','screen','multiply')),
          center_x REAL NOT NULL DEFAULT 0.5,
          center_y REAL NOT NULL DEFAULT 0.5,
          opacity REAL NOT NULL DEFAULT 1.0 CHECK(opacity >= 0.0 AND opacity <= 1.0),
          looping INTEGER NOT NULL DEFAULT 0,
          fps REAL NOT NULL DEFAULT 12.0,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );
        CREATE INDEX idx_vfx_effects_worktree
          ON vfx_effects(worktree_id, updated_at DESC);

        UPDATE animation_templates
        SET preferred_frames = max(min_frames, min(preferred_frames, max_frames))
        WHERE frame_mode = 'auto';

        INSERT INTO migrations(version, applied_at) VALUES (7, datetime('now'));
        "#,
        )
        .map_err(|error| CommandError::new("migration_failed", error.to_string()))?;
    Ok(())
}

pub(crate) fn migrate_v8(transaction: &Transaction<'_>) -> CommandResult<()> {
    transaction
        .execute_batch(
            r#"
        CREATE TABLE quality_reports (
          id TEXT PRIMARY KEY,
          project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
          worktree_id TEXT REFERENCES worktrees(id) ON DELETE SET NULL,
          animation_id TEXT NOT NULL REFERENCES animations(id) ON DELETE CASCADE,
          job_id TEXT REFERENCES background_jobs(id) ON DELETE SET NULL,
          status TEXT NOT NULL CHECK(status IN ('running','completed','failed','cancelled')),
          overall_score REAL NOT NULL DEFAULT 0.0,
          character_consistency_score REAL NOT NULL DEFAULT 0.0,
          motion_continuity_score REAL NOT NULL DEFAULT 0.0,
          frame_alignment_score REAL NOT NULL DEFAULT 0.0,
          weapon_consistency_score REAL NOT NULL DEFAULT 0.0,
          loop_quality_score REAL NOT NULL DEFAULT 0.0,
          transparency_score REAL NOT NULL DEFAULT 0.0,
          frame_count INTEGER NOT NULL CHECK(frame_count > 0),
          analyzer_version TEXT NOT NULL,
          created_at TEXT NOT NULL,
          completed_at TEXT,
          updated_at TEXT NOT NULL
        );
        CREATE INDEX idx_quality_reports_animation
          ON quality_reports(animation_id, created_at DESC);

        CREATE TABLE quality_checks (
          id TEXT PRIMARY KEY,
          report_id TEXT NOT NULL REFERENCES quality_reports(id) ON DELETE CASCADE,
          position INTEGER NOT NULL CHECK(position >= 0),
          check_type TEXT NOT NULL,
          frame_index INTEGER CHECK(frame_index IS NULL OR frame_index >= 0),
          comparison_frame_index INTEGER CHECK(comparison_frame_index IS NULL OR comparison_frame_index >= 0),
          severity TEXT NOT NULL CHECK(severity IN ('info','warning','error')),
          score REAL NOT NULL CHECK(score >= 0.0 AND score <= 100.0),
          message TEXT NOT NULL,
          metric_value REAL,
          metric_unit TEXT,
          repair_action TEXT,
          created_at TEXT NOT NULL,
          UNIQUE(report_id, position)
        );
        CREATE INDEX idx_quality_checks_report
          ON quality_checks(report_id, position);
        CREATE INDEX idx_quality_checks_frame
          ON quality_checks(report_id, frame_index, severity);

        CREATE TABLE quality_warnings (
          id TEXT PRIMARY KEY,
          report_id TEXT NOT NULL REFERENCES quality_reports(id) ON DELETE CASCADE,
          check_id TEXT NOT NULL REFERENCES quality_checks(id) ON DELETE CASCADE,
          acknowledged INTEGER NOT NULL DEFAULT 0,
          ignored INTEGER NOT NULL DEFAULT 0,
          resolution_note TEXT,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          UNIQUE(report_id, check_id)
        );

        CREATE TABLE frame_quality_cache (
          asset_id TEXT PRIMARY KEY REFERENCES assets(id) ON DELETE CASCADE,
          content_hash TEXT NOT NULL,
          width INTEGER NOT NULL,
          height INTEGER NOT NULL,
          alpha_min_x INTEGER,
          alpha_min_y INTEGER,
          alpha_max_x INTEGER,
          alpha_max_y INTEGER,
          centroid_x REAL,
          centroid_y REAL,
          alpha_coverage REAL NOT NULL,
          opaque_edge_pixels INTEGER NOT NULL,
          perceptual_hash TEXT NOT NULL,
          palette_signature TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );
        CREATE INDEX idx_frame_quality_cache_hash
          ON frame_quality_cache(content_hash);

        INSERT INTO migrations(version, applied_at) VALUES (8, datetime('now'));
        "#,
        )
        .map_err(|error| CommandError::new("migration_failed", error.to_string()))?;
    Ok(())
}
