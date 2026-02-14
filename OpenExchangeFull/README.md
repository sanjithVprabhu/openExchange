# OpenExchange

A white-label crypto options exchange built in Rust. This is an abstract, modular solution that can be customized through configuration.

## Project Structure

```
OpenExchangeFull/
├── Cargo.toml                 # Workspace root
├── cli/                       # CLI argument parsing
│   ├── Cargo.toml
│   └── src/lib.rs
├── config/                    # Config parsing & validation
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs            # Data structures
│       ├── defaults.rs       # Default values
│       ├── parser.rs         # YAML loading/saving
│       ├── substitution.rs   # Environment variable substitution
│       └── validator.rs      # All validation logic
├── src/                       # Main binary
│   ├── Cargo.toml
│   └── main.rs
└── master_config/
    └── master_config.yaml    # Example configuration
```

## Quick Start

### Build the Project

```bash
cd OpenExchangeFull
cargo build --release
```

### Available Commands

#### 1. Initialize a Configuration File
```bash
./target/release/openx init
# Or specify custom output path:
./target/release/openx init --output ./my_exchange.yaml
```

This creates a fully populated configuration file with all sensible defaults.

#### 2. Validate Configuration
```bash
./target/release/openx validate
# Or specify custom config path:
./target/release/openx validate --config ./my_exchange.yaml
```

Validates the configuration and displays:
- Defaults applied
- Warnings
- Errors (all collected and displayed at once)

#### 3. Start the Exchange
```bash
./target/release/openx start
# Or specify custom config path:
./target/release/openx start --config ./my_exchange.yaml
```

## Configuration

### Environment Variables

The configuration supports environment variable substitution using the syntax `${VAR_NAME}`:

```yaml
storage:
  postgres:
    host: "${INSTRUMENT_DB_HOST}"
    user: "${INSTRUMENT_DB_USER}"
    password: "${INSTRUMENT_DB_PASSWORD}"
```

Required environment variables:
- `INSTRUMENT_DB_HOST`, `INSTRUMENT_DB_USER`, `INSTRUMENT_DB_PASSWORD`
- `OMS_DB_HOST`, `OMS_DB_USER`, `OMS_DB_PASSWORD`
- `RISK_DB_HOST`, `RISK_DB_USER`, `RISK_DB_PASSWORD`
- `SUPABASE_URL`, `SUPABASE_ANON_KEY`, `SUPABASE_SERVICE_KEY` (if using Supabase)
- `REDIS_HOST`, `REDIS_PASSWORD` (if using Redis)
- `BINANCE_API_KEY`, `BINANCE_API_SECRET` (for market data)

### Configuration Sections

1. **exchange**: Exchange metadata (name, version, mode)
2. **instrument**: Supported assets and settlement currencies
3. **market_data**: Price feed providers and configuration
4. **expiry**: Options expiry schedules (daily, weekly, monthly, quarterly, yearly)
5. **storage**: Database configuration (Postgres/Supabase) and caching
6. **oms**: Order Management System settings
7. **matching_engine**: Order matching and execution settings
8. **risk_engine**: Margin, liquidation, and Greeks calculation

### Validation Rules

The configuration is validated against rules defined in `config_rules.md`:

- Exchange name and description are required
- Version must follow semver format (X.Y.Z)
- Mode must be one of: production, virtual, both
- At least one supported asset must be enabled
- At least one settlement currency must be enabled
- Exactly one settlement currency must be primary
- Market data provider type must be: websocket, grpc, or rest
- All expiry schedules must be defined
- Margin percentages must be between 0 and 1
- And many more...

## Architecture

### Module Dependencies

```
CLI Layer -> Config Layer (parsing, validation)
                |
                v
         Core Business Logic (future)
```

### Design Principles

1. **Config-driven**: All settings configurable via YAML
2. **Environment-aware**: Support for environment variable substitution
3. **Validation-first**: Comprehensive validation with clear error messages
4. **Default-rich**: Sensible defaults with logging when used
5. **Modular**: Clear separation between CLI, config, and core logic

## Development

### Running Tests

```bash
cargo test
```

### Project Features

- **CLI**: Subcommands (start, validate, init) with clap
- **Config**: YAML parsing with serde, comprehensive validation
- **Logging**: Full tracing support with INFO and DEBUG levels
- **Error Handling**: All errors collected and displayed at once
- **Defaults**: Extensive default values with warning logs

## License

MIT OR Apache-2.0

## Next Steps

To extend this project:

1. Add core business logic crates (matching engine, risk engine, etc.)
2. Implement storage adapters (Postgres, Redis, etc.)
3. Add gRPC/HTTP API endpoints
4. Implement WebSocket feeds
5. Add settlement and wallet services