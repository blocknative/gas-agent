# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.0.9] - 2025-01-22

### Added
- Initial release of Blocknative Gas Agent
- Real-time gas price estimation for the Gas Network
- EIP-1559 transaction handling and gas estimation
- Block polling based on exact block timing
- Parallel processing of model payloads
- Gas Network signature generation
- Prometheus metrics integration with OpenTelemetry
- RESTful API endpoints for gas price queries

### Changed
- Improved error handling by replacing anyhow with concrete ModelError types
- Enhanced pending floor settlement changed to Fast settlement
- Optimized network requests by reusing single Reqwest client
- Updated model functions to return FromBlock values

### Fixed
- Fixed clippy warnings and code quality issues
- Resolved EIP-1559 handling edge cases
- Improved model prediction error handling

### Technical Details
- Built with Rust and async/await patterns using Tokio
- Supports multiple blockchain networks and gas estimation models
- Comprehensive testing suite with unit and integration tests
- Production-ready with optimized release profile settings