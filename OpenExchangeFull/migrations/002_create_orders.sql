-- ============================================================================
-- OMS Database Schema
-- Migration: 002_create_orders.sql
-- ============================================================================

-- ============================================================================
-- ENUM TYPES
-- ============================================================================

-- Order side enum
DO $$ BEGIN
    CREATE TYPE order_side AS ENUM ('buy', 'sell');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- Order type enum
DO $$ BEGIN
    CREATE TYPE order_type AS ENUM ('limit', 'market');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- Time in force enum
DO $$ BEGIN
    CREATE TYPE time_in_force AS ENUM ('gtc', 'ioc', 'fok', 'day');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- Order status enum
DO $$ BEGIN
    CREATE TYPE order_status AS ENUM (
        'pending_risk', 'open', 'partially_filled', 
        'filled', 'cancelled', 'rejected', 'expired'
    );
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- ============================================================================
-- ORDERS TABLE (PRODUCTION)
-- ============================================================================

CREATE TABLE IF NOT EXISTS orders_prod (
    order_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL,
    instrument_id VARCHAR(128) NOT NULL,
    side order_side NOT NULL,
    order_type order_type NOT NULL DEFAULT 'limit',
    time_in_force time_in_force NOT NULL DEFAULT 'gtc',
    price DECIMAL(24, 8),
    quantity INTEGER NOT NULL CHECK (quantity > 0),
    filled_quantity INTEGER NOT NULL DEFAULT 0 CHECK (filled_quantity >= 0),
    avg_fill_price DECIMAL(24, 8),
    status order_status NOT NULL DEFAULT 'pending_risk',
    client_order_id VARCHAR(64),
    risk_approved_at TIMESTAMPTZ,
    risk_rejection_reason TEXT,
    required_margin DECIMAL(24, 8),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT orders_prod_fill_check CHECK (filled_quantity <= quantity),
    CONSTRAINT orders_prod_limit_price CHECK (
        (order_type = 'limit' AND price IS NOT NULL AND price > 0) OR
        (order_type = 'market' AND price IS NULL)
    )
);

CREATE INDEX IF NOT EXISTS idx_orders_prod_user_created ON orders_prod(user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_orders_prod_user_status ON orders_prod(user_id, status);
CREATE INDEX IF NOT EXISTS idx_orders_prod_instrument ON orders_prod(instrument_id);
CREATE INDEX IF NOT EXISTS idx_orders_prod_status ON orders_prod(status) 
    WHERE status IN ('open', 'partially_filled', 'pending_risk');
CREATE INDEX IF NOT EXISTS idx_orders_prod_client_order_id ON orders_prod(user_id, client_order_id) 
    WHERE client_order_id IS NOT NULL;

-- Trigger for updated_at
CREATE OR REPLACE FUNCTION update_orders_prod_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trigger_orders_prod_updated_at ON orders_prod;
CREATE TRIGGER trigger_orders_prod_updated_at
    BEFORE UPDATE ON orders_prod
    FOR EACH ROW
    EXECUTE FUNCTION update_orders_prod_timestamp();

-- ============================================================================
-- ORDER FILLS TABLE (PRODUCTION)
-- ============================================================================

CREATE TABLE IF NOT EXISTS order_fills_prod (
    fill_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    order_id UUID NOT NULL REFERENCES orders_prod(order_id) ON DELETE CASCADE,
    trade_id UUID NOT NULL,
    quantity INTEGER NOT NULL CHECK (quantity > 0),
    price DECIMAL(24, 8) NOT NULL CHECK (price > 0),
    counterparty_order_id UUID,
    fee DECIMAL(24, 8) NOT NULL DEFAULT 0,
    fee_currency VARCHAR(10) NOT NULL DEFAULT 'USDT',
    is_maker BOOLEAN NOT NULL DEFAULT FALSE,
    executed_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_order_fills_prod_order ON order_fills_prod(order_id, executed_at);
CREATE INDEX IF NOT EXISTS idx_order_fills_prod_trade ON order_fills_prod(trade_id);

-- ============================================================================
-- ORDERS TABLE (VIRTUAL)
-- ============================================================================

CREATE TABLE IF NOT EXISTS orders_virtual (
    order_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL,
    instrument_id VARCHAR(128) NOT NULL,
    side order_side NOT NULL,
    order_type order_type NOT NULL DEFAULT 'limit',
    time_in_force time_in_force NOT NULL DEFAULT 'gtc',
    price DECIMAL(24, 8),
    quantity INTEGER NOT NULL CHECK (quantity > 0),
    filled_quantity INTEGER NOT NULL DEFAULT 0 CHECK (filled_quantity >= 0),
    avg_fill_price DECIMAL(24, 8),
    status order_status NOT NULL DEFAULT 'pending_risk',
    client_order_id VARCHAR(64),
    risk_approved_at TIMESTAMPTZ,
    risk_rejection_reason TEXT,
    required_margin DECIMAL(24, 8),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT orders_virtual_fill_check CHECK (filled_quantity <= quantity),
    CONSTRAINT orders_virtual_limit_price CHECK (
        (order_type = 'limit' AND price IS NOT NULL AND price > 0) OR
        (order_type = 'market' AND price IS NULL)
    )
);

CREATE INDEX IF NOT EXISTS idx_orders_virtual_user_created ON orders_virtual(user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_orders_virtual_status ON orders_virtual(status) 
    WHERE status IN ('open', 'partially_filled', 'pending_risk');

DROP TRIGGER IF EXISTS trigger_orders_virtual_updated_at ON orders_virtual;
CREATE TRIGGER trigger_orders_virtual_updated_at
    BEFORE UPDATE ON orders_virtual
    FOR EACH ROW
    EXECUTE FUNCTION update_orders_prod_timestamp();

-- ============================================================================
-- ORDER FILLS TABLE (VIRTUAL)
-- ============================================================================

CREATE TABLE IF NOT EXISTS order_fills_virtual (
    fill_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    order_id UUID NOT NULL REFERENCES orders_virtual(order_id) ON DELETE CASCADE,
    trade_id UUID NOT NULL,
    quantity INTEGER NOT NULL CHECK (quantity > 0),
    price DECIMAL(24, 8) NOT NULL CHECK (price > 0),
    counterparty_order_id UUID,
    fee DECIMAL(24, 8) NOT NULL DEFAULT 0,
    fee_currency VARCHAR(10) NOT NULL DEFAULT 'USDT',
    is_maker BOOLEAN NOT NULL DEFAULT FALSE,
    executed_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_order_fills_virtual_order ON order_fills_virtual(order_id, executed_at);

-- ============================================================================
-- ORDERS TABLE (STATIC)
-- ============================================================================

CREATE TABLE IF NOT EXISTS orders_static (
    order_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL,
    instrument_id VARCHAR(128) NOT NULL,
    side order_side NOT NULL,
    order_type order_type NOT NULL DEFAULT 'limit',
    time_in_force time_in_force NOT NULL DEFAULT 'gtc',
    price DECIMAL(24, 8),
    quantity INTEGER NOT NULL CHECK (quantity > 0),
    filled_quantity INTEGER NOT NULL DEFAULT 0 CHECK (filled_quantity >= 0),
    avg_fill_price DECIMAL(24, 8),
    status order_status NOT NULL DEFAULT 'pending_risk',
    client_order_id VARCHAR(64),
    risk_approved_at TIMESTAMPTZ,
    risk_rejection_reason TEXT,
    required_margin DECIMAL(24, 8),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT orders_static_fill_check CHECK (filled_quantity <= quantity)
);

CREATE INDEX IF NOT EXISTS idx_orders_static_user_created ON orders_static(user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_orders_static_status ON orders_static(status) 
    WHERE status IN ('open', 'partially_filled', 'pending_risk');

DROP TRIGGER IF EXISTS trigger_orders_static_updated_at ON orders_static;
CREATE TRIGGER trigger_orders_static_updated_at
    BEFORE UPDATE ON orders_static
    FOR EACH ROW
    EXECUTE FUNCTION update_orders_prod_timestamp();

-- ============================================================================
-- ORDER FILLS TABLE (STATIC)
-- ============================================================================

CREATE TABLE IF NOT EXISTS order_fills_static (
    fill_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    order_id UUID NOT NULL REFERENCES orders_static(order_id) ON DELETE CASCADE,
    trade_id UUID NOT NULL,
    quantity INTEGER NOT NULL CHECK (quantity > 0),
    price DECIMAL(24, 8) NOT NULL CHECK (price > 0),
    counterparty_order_id UUID,
    fee DECIMAL(24, 8) NOT NULL DEFAULT 0,
    fee_currency VARCHAR(10) NOT NULL DEFAULT 'USDT',
    is_maker BOOLEAN NOT NULL DEFAULT FALSE,
    executed_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_order_fills_static_order ON order_fills_static(order_id, executed_at);

-- ============================================================================
-- HELPER VIEWS
-- ============================================================================

-- View for active orders with fill summary (Production)
CREATE OR REPLACE VIEW v_active_orders_prod AS
SELECT 
    o.order_id,
    o.user_id,
    o.instrument_id,
    o.side,
    o.order_type,
    o.time_in_force,
    o.price,
    o.quantity,
    o.filled_quantity,
    o.quantity - o.filled_quantity AS remaining_quantity,
    o.avg_fill_price,
    o.status,
    o.created_at,
    o.updated_at,
    COUNT(f.fill_id) AS fill_count,
    MAX(f.executed_at) AS last_fill_at
FROM orders_prod o
LEFT JOIN order_fills_prod f ON o.order_id = f.order_id
WHERE o.status IN ('open', 'partially_filled')
GROUP BY o.order_id;

-- View for order history with all fills (Production)
CREATE OR REPLACE VIEW v_order_history_prod AS
SELECT 
    o.order_id,
    o.user_id,
    o.instrument_id,
    o.side,
    o.price AS order_price,
    o.quantity,
    o.filled_quantity,
    o.avg_fill_price,
    o.status,
    o.created_at AS order_created_at,
    f.fill_id,
    f.trade_id,
    f.quantity AS fill_quantity,
    f.price AS fill_price,
    f.fee,
    f.executed_at AS fill_executed_at
FROM orders_prod o
LEFT JOIN order_fills_prod f ON o.order_id = f.order_id
ORDER BY o.created_at DESC, f.executed_at ASC;
