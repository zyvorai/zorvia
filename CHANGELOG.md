# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2024-02-05

### Added

#### Core VM Management
- `create` command - Create VMs from templates or configuration files
- `list` command - List VMs with table, YAML, or JSON output
- `get` command - Get detailed VM information
- `delete` command - Delete VMs with confirmation prompt
- `start` command - Start stopped VMs
- `stop` command - Stop running VMs
- `restart` command - Restart VMs
- Multi-namespace support with `--all-namespaces` flag

#### Templates & Configuration
- 6 built-in VM templates (Ubuntu, CentOS, Fedora, Debian, RHEL, Windows)
- `templates` command - List available templates
- `template` command - Show template details
- `generate` command - Generate VM manifests (VMConfig or KubeVirt CRD)
- `validate` command - Validate VM configuration files
- Cloud-init support for VM customization
- Builder pattern for programmatic VM configuration

#### Advanced Features (Innovation Phase)
- `status` command - Detailed VM status with watch mode
- `clone` command - Clone existing VMs
- `resources` command - Cluster-wide resource usage summary
- `export` command - Export VM configurations for backup
- `wizard` command - Interactive VM creation with prompts
- `batch` command - Create multiple VMs from batch configuration files
- Progress bars for batch operations
- Real-time monitoring with watch mode

#### Kubernetes Integration
- Full KubeVirt VirtualMachine CRD support
- VMConfig to KubeVirt manifest converter
- Complete CRUD operations via Kubernetes API
- PVC creation support
- VM lifecycle management
- Status tracking and condition monitoring

#### Developer Features
- Comprehensive validation system (names, resources, configs)
- Custom error types with `thiserror`
- 31 unit and integration tests (100% pass rate)
- Library API for programmatic usage
- Zero unsafe code
- Zero compilation warnings

#### Documentation
- Complete README with quick start guide
- DEVELOPMENT.md with roadmap and status
- IMPLEMENTATION_SUMMARY.md with implementation details
- ADVANCED_FEATURES.md comprehensive feature guide
- INNOVATION_SUMMARY.md with innovation highlights
- FINAL_DELIVERY.md complete delivery report
- CONTRIBUTING.md for contributors
- 8 example configuration files
- API documentation

#### CI/CD
- GitHub Actions workflow
- Automated testing on push/PR
- Code formatting checks (rustfmt)
- Linting with clippy
- Multi-platform builds (Ubuntu, macOS)
- Dependency caching

#### Licensing
- Dual-licensed under Apache-2.0
- Permissive and enterprise-friendly

### Technical Details

#### Dependencies
- kube 0.95 - Kubernetes client
- k8s-openapi 0.23 - Kubernetes API types
- schemars 0.8 - JSON Schema support
- serde 1.0 - Serialization framework
- serde_yaml 0.9 - YAML support
- serde_json 1.0 - JSON support
- tokio 1.0 - Async runtime
- clap 4.5 - CLI framework
- anyhow 1.0 - Error handling
- thiserror 1.0 - Error derive macros
- regex 1.10 - Validation
- once_cell 1.19 - Lazy statics
- dialoguer 0.11 - Interactive prompts
- indicatif 0.17 - Progress bars
- chrono 0.4 - Date/time handling
- log 0.4 - Logging facade
- env_logger 0.11 - Logger implementation

#### Architecture
- 8 modules: cli, config, kube, templates, storage, network, output, utils
- 25 Rust source files
- ~4500 lines of code
- Modular, maintainable structure

### Statistics
- Commands: 17 (11 core + 6 advanced)
- Templates: 6 pre-built templates
- Tests: 31 (100% pass rate)
- Documentation: 6 comprehensive guides
- Examples: 8 configuration files

[Unreleased]: https://github.com/zyvorai/zorvia/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/zyvorai/zorvia/releases/tag/v0.1.0
