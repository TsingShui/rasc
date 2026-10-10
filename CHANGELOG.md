# Changelog

## [1.0.0](https://github.com/TsingShui/rasc/compare/rasc-v0.4.0...rasc-v1.0.0) (2026-10-10)


### ⚠ BREAKING CHANGES

* Java-bytecode JAR/AAR inputs and --d8/--no-d8 options are no longer supported. APK, DEX, and archives containing root DEX entries remain supported.

### Bug Fixes

* remove implicit d8 conversion for JAR and AAR inputs ([bbd3f0b](https://github.com/TsingShui/rasc/commit/bbd3f0b500f66951a47a8a5acac4f1dddf2dbeef))

## [0.4.0](https://github.com/TsingShui/rasc/compare/rasc-v0.3.0...rasc-v0.4.0) (2026-10-09)


### Features

* distribute Linux x86_64 musl binaries ([153efdf](https://github.com/TsingShui/rasc/commit/153efdfe01127da3c88f683ed49325e0d9de5b00))

## [0.3.0](https://github.com/TsingShui/rasc/compare/rasc-v0.2.0...rasc-v0.3.0) (2026-10-08)


### Features

* support JAR and AAR inputs via d8 ([0f2d50c](https://github.com/TsingShui/rasc/commit/0f2d50c30dee15aba86c8844a5093ef1cdd60696))

## [0.2.0](https://github.com/TsingShui/rasc/compare/rasc-v0.1.2...rasc-v0.2.0) (2026-10-05)


### Features

* add persistent MCP analysis server ([1303b20](https://github.com/TsingShui/rasc/commit/1303b20635537c592f6b641895c079ea3d391cce))

## [0.1.2](https://github.com/TsingShui/rasc/compare/rasc-v0.1.1...rasc-v0.1.2) (2026-09-28)


### Bug Fixes

* **release:** build component-prefixed release tags ([cbbe7f2](https://github.com/TsingShui/rasc/commit/cbbe7f2fdd76b5a6a6a36b54c2cf8e30f83a00a6))

## [0.1.1](https://github.com/TsingShui/rasc/compare/rasc-v0.1.0...rasc-v0.1.1) (2026-09-28)


### Bug Fixes

* test release-please version bump ([5cac6ab](https://github.com/TsingShui/rasc/commit/5cac6abaf3126249a19d0502a9f5a33f85c9c397))
