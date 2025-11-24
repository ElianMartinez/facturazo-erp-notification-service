-- Initial schema for PDF Services with Dominican Republic fiscal compliance
-- SQLite database with multi-tenant support

-- Enable foreign key constraints
PRAGMA foreign_keys = ON;

-- Documents table
CREATE TABLE IF NOT EXISTS documents (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    document_type TEXT NOT NULL CHECK (document_type IN ('invoice', 'quotation', 'report', 'receipt', 'credit_note')),
    document_number TEXT NOT NULL,

    -- Storage information
    storage_path TEXT,
    storage_url TEXT,
    file_size INTEGER,
    mime_type TEXT DEFAULT 'application/pdf',

    -- Metadata
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'processing', 'completed', 'failed', 'archived')),
    error_message TEXT,

    -- Timestamps
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMP,

    -- Indexes for multi-tenant queries
    UNIQUE(tenant_id, document_type, document_number)
);

-- Invoices table (Dominican Republic fiscal compliance)
CREATE TABLE IF NOT EXISTS invoices (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    invoice_number TEXT NOT NULL,
    document_id TEXT REFERENCES documents(id),

    -- NCF (Número de Comprobante Fiscal)
    ncf TEXT NOT NULL,
    ncf_serie CHAR(1) NOT NULL,
    ncf_tipo_code INTEGER NOT NULL,
    ncf_sequence INTEGER NOT NULL,
    ncf_expiry_date TIMESTAMP NOT NULL,

    -- Seller information
    seller_company_name TEXT NOT NULL,
    seller_trade_name TEXT,
    seller_rnc TEXT NOT NULL,
    seller_address TEXT NOT NULL,
    seller_phone TEXT NOT NULL,
    seller_email TEXT NOT NULL,

    -- Customer information
    customer_name TEXT NOT NULL,
    customer_tax_id TEXT,
    customer_tax_id_type TEXT CHECK (customer_tax_id_type IN ('RNC', 'Cedula')),
    customer_address TEXT,
    customer_phone TEXT,
    customer_email TEXT,

    -- Financial details
    currency TEXT NOT NULL DEFAULT 'DOP',
    exchange_rate REAL,
    subtotal REAL NOT NULL,
    discount_amount REAL DEFAULT 0,
    itbis_amount REAL NOT NULL,
    total_amount REAL NOT NULL,

    -- Payment information
    payment_terms TEXT NOT NULL,
    payment_method TEXT,
    issue_date TIMESTAMP NOT NULL,
    due_date TIMESTAMP,
    paid_date TIMESTAMP,

    -- Status
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'issued', 'sent', 'paid', 'overdue', 'cancelled')),

    -- Metadata
    notes TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    created_by TEXT,

    UNIQUE(tenant_id, invoice_number),
    UNIQUE(ncf)
);

-- Invoice items table
CREATE TABLE IF NOT EXISTS invoice_items (
    id TEXT PRIMARY KEY,
    invoice_id TEXT NOT NULL REFERENCES invoices(id) ON DELETE CASCADE,
    sequence_number INTEGER NOT NULL,

    description TEXT NOT NULL,
    quantity REAL NOT NULL,
    unit_of_measure TEXT NOT NULL,
    unit_price REAL NOT NULL,

    discount_percentage REAL DEFAULT 0,
    discount_amount REAL DEFAULT 0,

    itbis_rate REAL NOT NULL,
    itbis_amount REAL NOT NULL,

    subtotal REAL NOT NULL,
    total REAL NOT NULL,

    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,

    UNIQUE(invoice_id, sequence_number)
);

-- NCF sequences table (for managing fiscal sequences)
CREATE TABLE IF NOT EXISTS ncf_sequences (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    serie CHAR(1) NOT NULL,
    tipo_code INTEGER NOT NULL,
    current_sequence INTEGER NOT NULL DEFAULT 1,
    max_sequence INTEGER NOT NULL,
    expiry_date TIMESTAMP NOT NULL,

    is_active BOOLEAN DEFAULT 1,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,

    UNIQUE(tenant_id, serie, tipo_code, is_active)
);

-- Notifications table
CREATE TABLE IF NOT EXISTS notifications (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    document_id TEXT REFERENCES documents(id),

    channel TEXT NOT NULL CHECK (channel IN ('email', 'whatsapp', 'in_app', 'sms')),
    recipient TEXT NOT NULL,
    recipient_name TEXT,

    subject TEXT,
    content TEXT NOT NULL,
    template_id TEXT,

    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'sent', 'delivered', 'failed', 'bounced')),
    error_message TEXT,

    sent_at TIMESTAMP,
    delivered_at TIMESTAMP,

    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Templates table
CREATE TABLE IF NOT EXISTS templates (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    template_type TEXT NOT NULL CHECK (template_type IN ('document', 'email', 'whatsapp')),
    template_name TEXT NOT NULL,
    version TEXT NOT NULL DEFAULT '1.0.0',

    content TEXT NOT NULL,
    metadata TEXT, -- JSON string with template metadata

    is_active BOOLEAN DEFAULT 1,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,

    UNIQUE(tenant_id, template_type, template_name, version)
);

-- Audit log table
CREATE TABLE IF NOT EXISTS audit_logs (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    action TEXT NOT NULL,

    user_id TEXT,
    changes TEXT, -- JSON string with before/after values
    ip_address TEXT,
    user_agent TEXT,

    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Create indexes for performance
CREATE INDEX idx_documents_tenant_status ON documents(tenant_id, status);
CREATE INDEX idx_documents_created_at ON documents(created_at);
CREATE INDEX idx_invoices_tenant_status ON invoices(tenant_id, status);
CREATE INDEX idx_invoices_ncf ON invoices(ncf);
CREATE INDEX idx_invoices_due_date ON invoices(due_date);
CREATE INDEX idx_notifications_tenant_status ON notifications(tenant_id, status);
CREATE INDEX idx_notifications_document ON notifications(document_id);
CREATE INDEX idx_audit_logs_tenant_entity ON audit_logs(tenant_id, entity_type, entity_id);
CREATE INDEX idx_audit_logs_created_at ON audit_logs(created_at);

-- Trigger to update updated_at timestamp
CREATE TRIGGER update_documents_timestamp
AFTER UPDATE ON documents
BEGIN
    UPDATE documents SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;

CREATE TRIGGER update_invoices_timestamp
AFTER UPDATE ON invoices
BEGIN
    UPDATE invoices SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;

CREATE TRIGGER update_notifications_timestamp
AFTER UPDATE ON notifications
BEGIN
    UPDATE notifications SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;

CREATE TRIGGER update_templates_timestamp
AFTER UPDATE ON templates
BEGIN
    UPDATE templates SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;

CREATE TRIGGER update_ncf_sequences_timestamp
AFTER UPDATE ON ncf_sequences
BEGIN
    UPDATE ncf_sequences SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;