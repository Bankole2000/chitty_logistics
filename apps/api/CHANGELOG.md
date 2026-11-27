# Changelog — api

All notable changes to this app are recorded here. Format based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow [Semantic Versioning](https://semver.org/) and are tagged `api-vX.Y.Z`.

## [Unreleased]
### Added
- Server skeleton: health check at `GET /health` including the database, request IDs (`x-request-id`), the standard error body, CORS from `API_CORS_ORIGINS`, migrations with PostGIS enabled.
- OpenAPI 3.1 contract generated from the code with utoipa, committed as `packages/api-contract/openapi.json`, documenting `GET /health` and the standard error body (`ErrorBody`). New subcommand `api openapi --out <file>` writes it and `api openapi --check [<file>]` fails when it is out of date; neither needs a database. A Bruno collection in `packages/api-contract/bruno/` has a request per endpoint.
- Staff roles and permissions: staff accounts, roles (operations, admin, finance, HR with drafted permission sets), permission keys synced to the database on startup, a permission check that answers 403 `forbidden`, and append-only audit entries written in the same transaction as the change they record.
### Changed
### Fixed
### Removed
