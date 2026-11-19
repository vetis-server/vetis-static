# vetis-static

[![Crates.io downloads](https://img.shields.io/crates/d/vetis-static)](https://crates.io/crates/vetis-static) [![crates.io](https://img.shields.io/crates/v/vetis-static?style=flat-square)](https://crates.io/crates/vetis-static) [![Build Status](https://github.com/vetis-server/vetis-static/actions/workflows/rust.yml/badge.svg?event=push)](https://github.com/vetis-server/vetis-static/actions/workflows/rust.yml) ![Crates.io MSRV](https://img.shields.io/crates/msrv/vetis-static) [![Documentation](https://docs.rs/vetis-static/badge.svg)](https://docs.rs/vetis-static/latest/vetis-static) [![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/vetis-server/vetis-static/blob/main/LICENSE.md)  [![codecov](https://codecov.io/gh/vetis-server/vetis-static/graph/badge.svg?token=T0HSBAPVSI)](https://codecov.io/gh/vetis-server/vetis-static)

Static file serving support for the vetis HTTP server framework. This module provides efficient static file serving with configurable caching, support for multiple async runtimes, and flexible path configuration.

## Features

- Serve static files with configurable extensions and directories
- Built-in caching with TTL (time-to-live) and TTI (time-to-idle) support
- Support for multiple async runtimes: Tokio, Smol, and Compio
- Configurable index files for directory requests
- MIME type detection
- ETag support for cache validation
- Memory and disk-based file storage options

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
vetis-static = "0.1.1-beta.2"
```

### Runtime Features

Choose the appropriate runtime feature for your project:

- `runtime-tokio` (default): Use with Tokio runtime
- `runtime-smol`: Use with Smol runtime
- `runtime-compio`: Use with Compio runtime

## Configuration

### Using API

Configure static file serving using the builder pattern:

```rust,ignore
use vetis_static::StaticHookConfig;
use vetis_matchers::request::{PathMatcher, PathMatcherConfig};

let config = StaticHookConfig::builder()
    .matcher(PathMatcher::new(
        PathMatcherConfig::builder()
            .add_path("/static*")
            .build()?,
    ))
    .extensions(r"\.(html|css|js|png|jpg)$")
    .directory("./public")
    .index_files(vec!["index.html".to_string()])
    .build()?;
```

### vetis.yaml

```yaml
name: static
matcher:
  type: "path"
  paths:
    - "/static*"
extensions: \.(html|css|js|png|jpg)$
directory: ./public
index_files: [index.html]
```

### Configuration Options

#### StaticHookConfig

- **matcher**: If set, this hook will be only called if request meet the criteria defined by him, otherwise, all requests will call this hook (optional)
- **extensions**: Comma-separated list of file extensions to serve (required)
- **directory**: The filesystem directory containing static files (required)
- **index_files**: Optional list of index file names to serve for directory requests

## Runtime-Specific Modules

Based on your selected runtime feature, import the appropriate module:

```rust
// For Tokio runtime
#[cfg(feature = "runtime-tokio")]
use vetis_static::tokio;

// For Smol runtime
#[cfg(feature = "runtime-smol")]
use vetis_static::smol;

// For Compio runtime
#[cfg(feature = "runtime-compio")]
use vetis_static::compio;
```

## License

Licensed under either of

- Apache License, Version 2.0
  (LICENSE-APACHE or <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT license
  (LICENSE-MIT or <https://opensource.org/licenses/MIT>)

at your option.

## Author

Rogerio Pereira Araujo <rogerio.araujo@gmail.com>
