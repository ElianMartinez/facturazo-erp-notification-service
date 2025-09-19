-- Create documents table
CREATE TABLE IF NOT EXISTS documents (
    id UUID PRIMARY KEY,
    status VARCHAR(50) NOT NULL,
    document_type VARCHAR(50) NOT NULL,
    template_id VARCHAR(100) NOT NULL,
    format VARCHAR(20) NOT NULL,
    priority VARCHAR(20) NOT NULL DEFAULT 'normal',

    -- User and org info
    user_id VARCHAR(100) NOT NULL,
    organization_id VARCHAR(100) NOT NULL,

    -- URLs and paths
    url TEXT,
    s3_key TEXT,

    -- Metadata
    error TEXT,
    processing_time_ms BIGINT,
    file_size_bytes BIGINT,
    row_count INTEGER,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ,

    -- Indexing
    INDEX idx_user_id (user_id),
    INDEX idx_organization_id (organization_id),
    INDEX idx_status (status),
    INDEX idx_created_at (created_at)
);

-- Create document_events table for audit trail
CREATE TABLE IF NOT EXISTS document_events (
    id SERIAL PRIMARY KEY,
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    event_type VARCHAR(50) NOT NULL,
    event_data JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    INDEX idx_document_id (document_id),
    INDEX idx_event_type (event_type)
);

-- Create templates table
CREATE TABLE IF NOT EXISTS templates (
    id VARCHAR(100) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    template_type VARCHAR(50) NOT NULL, -- 'invoice', 'report', 'certificate', etc.
    format VARCHAR(20) NOT NULL, -- 'typst', 'jinja', 'html'
    content TEXT NOT NULL,
    schema JSONB,
    version INTEGER NOT NULL DEFAULT 1,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_by VARCHAR(100),

    INDEX idx_template_type (template_type),
    INDEX idx_is_active (is_active)
);

-- Create rate_limits table for tracking
CREATE TABLE IF NOT EXISTS rate_limits (
    user_id VARCHAR(100) NOT NULL,
    minute_bucket TIMESTAMPTZ NOT NULL,
    request_count INTEGER NOT NULL DEFAULT 0,

    PRIMARY KEY (user_id, minute_bucket),
    INDEX idx_minute_bucket (minute_bucket)
);

-- Create usage_statistics table
CREATE TABLE IF NOT EXISTS usage_statistics (
    id SERIAL PRIMARY KEY,
    organization_id VARCHAR(100) NOT NULL,
    date DATE NOT NULL,
    document_type VARCHAR(50) NOT NULL,
    format VARCHAR(20) NOT NULL,
    count INTEGER NOT NULL DEFAULT 0,
    total_processing_time_ms BIGINT DEFAULT 0,
    total_size_bytes BIGINT DEFAULT 0,
    failed_count INTEGER DEFAULT 0,

    UNIQUE KEY unique_org_date_type (organization_id, date, document_type, format),
    INDEX idx_organization_date (organization_id, date)
);

-- Function to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Triggers for updated_at
CREATE TRIGGER update_documents_updated_at BEFORE UPDATE ON documents
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_templates_updated_at BEFORE UPDATE ON templates
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();