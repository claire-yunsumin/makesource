-- 초기 스키마 (TAD §3.1)

CREATE TABLE generations (
  id TEXT PRIMARY KEY,             -- uuid
  created_at INTEGER NOT NULL,     -- unix ms
  image_path TEXT NOT NULL,
  thumb_path TEXT NOT NULL,
  keyword_ko TEXT,
  prompt_final TEXT NOT NULL,
  negative TEXT,
  preset_id TEXT,
  preset_version INTEGER,
  style_id TEXT,                   -- essence 또는 lora 스타일
  seed INTEGER NOT NULL,
  steps INTEGER,
  cfg REAL,
  width INTEGER,
  height INTEGER,
  model TEXT,
  favorite INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_gen_created ON generations (created_at DESC);

CREATE TABLE training_jobs (
  id TEXT PRIMARY KEY,
  style_id TEXT NOT NULL,
  status TEXT NOT NULL,            -- queued|captioning|training|done|failed|canceled
  progress REAL NOT NULL DEFAULT 0,
  eta_seconds INTEGER,
  params_json TEXT,
  error TEXT,
  started_at INTEGER,
  finished_at INTEGER
);
