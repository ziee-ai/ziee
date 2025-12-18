-- Desktop Settings Table
-- Stores key-value settings for the desktop application

CREATE TABLE IF NOT EXISTS desktop_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for faster lookups
CREATE INDEX IF NOT EXISTS idx_desktop_settings_key ON desktop_settings(key);

-- Trigger to auto-update updated_at
CREATE OR REPLACE FUNCTION update_desktop_settings_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE TRIGGER trigger_desktop_settings_updated_at
    BEFORE UPDATE ON desktop_settings
    FOR EACH ROW
    EXECUTE FUNCTION update_desktop_settings_updated_at();
