-- =============================================================================
-- OpenExchange Instrument Store Migration
-- File: migrations/001_create_instruments.sql
-- =============================================================================

-- =============================================================================
-- INSTRUMENTS TABLE (Production)
-- =============================================================================

CREATE TABLE IF NOT EXISTS instruments_prod (
    -- Primary key
    id                  UUID PRIMARY KEY,
    
    -- Symbol: BTC-20260315-50000-C
    symbol              VARCHAR(64) UNIQUE NOT NULL,
    
    -- Underlying asset
    underlying_symbol   VARCHAR(16) NOT NULL,
    underlying_name     VARCHAR(64) NOT NULL,
    underlying_decimals INTEGER NOT NULL,
    
    -- Option details
    option_type         VARCHAR(4) NOT NULL CHECK (option_type IN ('call', 'put')),
    exercise_style      VARCHAR(16) NOT NULL DEFAULT 'european' 
                        CHECK (exercise_style IN ('european', 'american')),
    
    -- Strike price
    strike_value        DECIMAL(32, 8) NOT NULL,
    strike_decimals     INTEGER NOT NULL,
    
    -- Expiry (timestamp with timezone)
    expiry              TIMESTAMPTZ NOT NULL,
    
    -- Contract specifications
    settlement_currency VARCHAR(16) NOT NULL,
    contract_size       DECIMAL(32, 16) NOT NULL,
    tick_size           DECIMAL(32, 16) NOT NULL,
    min_order_size      BIGINT NOT NULL,
    
    -- Status
    status              VARCHAR(16) NOT NULL DEFAULT 'active' 
                        CHECK (status IN ('active', 'inactive', 'expired', 'settled', 'pending')),
    
    -- Timestamps
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- Constraints
    CONSTRAINT instruments_prod_positive_strike CHECK (strike_value > 0),
    CONSTRAINT instruments_prod_positive_contract CHECK (contract_size > 0),
    CONSTRAINT instruments_prod_positive_tick CHECK (tick_size > 0)
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_instruments_prod_underlying ON instruments_prod(underlying_symbol);
CREATE INDEX IF NOT EXISTS idx_instruments_prod_status ON instruments_prod(status);
CREATE INDEX IF NOT EXISTS idx_instruments_prod_expiry ON instruments_prod(expiry);
CREATE INDEX IF NOT EXISTS idx_instruments_prod_strike ON instruments_prod(strike_value);
CREATE INDEX IF NOT EXISTS idx_instruments_prod_option_type ON instruments_prod(option_type);
CREATE INDEX IF NOT EXISTS idx_instruments_prod_underlying_status ON instruments_prod(underlying_symbol, status);
CREATE INDEX IF NOT EXISTS idx_instruments_prod_expiry_status ON instruments_prod(expiry, status);

-- Updated at trigger
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

CREATE TRIGGER update_instruments_prod_updated_at 
    BEFORE UPDATE ON instruments_prod 
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- =============================================================================
-- Virtual and Static tables (identical structure)
-- =============================================================================

CREATE TABLE IF NOT EXISTS instruments_virtual (LIKE instruments_prod INCLUDING ALL);
CREATE TRIGGER update_instruments_virtual_updated_at 
    BEFORE UPDATE ON instruments_virtual 
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TABLE IF NOT EXISTS instruments_static (LIKE instruments_prod INCLUDING ALL);
CREATE TRIGGER update_instruments_static_updated_at 
    BEFORE UPDATE ON instruments_static 
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- =============================================================================
-- GENERATION STATE TABLE
-- Tracks displacement trigger reference points per asset per environment
-- =============================================================================

CREATE TABLE IF NOT EXISTS generation_state (
    id                      SERIAL PRIMARY KEY,
    environment             VARCHAR(16) NOT NULL CHECK (environment IN ('prod', 'virtual', 'static')),
    asset_symbol            VARCHAR(16) NOT NULL,
    
    -- Current reference point (last trigger that was crossed)
    upper_reference         DECIMAL(32, 8) NOT NULL,
    lower_reference         DECIMAL(32, 8) NOT NULL,
    
    -- Current triggers (calculated from reference + disp)
    upper_trigger           DECIMAL(32, 8) NOT NULL,
    lower_trigger           DECIMAL(32, 8) NOT NULL,
    
    -- Current bounds (calculated from reference + bound)
    max_strike              DECIMAL(32, 8) NOT NULL,
    min_strike              DECIMAL(32, 8) NOT NULL,
    
    -- Last processed spot price
    last_spot_price         DECIMAL(32, 8) NOT NULL,
    
    -- Timestamps
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    UNIQUE(environment, asset_symbol)
);

CREATE TRIGGER update_generation_state_updated_at 
    BEFORE UPDATE ON generation_state 
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
