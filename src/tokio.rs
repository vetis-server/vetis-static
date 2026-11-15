use crate::{StaticFile, StaticFileMetadata, StaticHookConfig, format_date};
use cel::Context;
use http::{HeaderMap, HeaderValue, StatusCode};
use hyper_body_utils::HttpBody;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
#[cfg(windows)]
use std::os::windows::fs::MetadataExt;
use std::path::PathBuf;
use tokio::fs::File;
use vetis::{
    Request, Response, Str, VetisFutureResult, VetisResult,
    errors::{HostError, RuntimeError, VetisError},
    hook::{Hook, HookConfig, HookPhase},
    matcher::Matcher,
};

/// Static path
pub struct StaticHook {
    matcher: Option<Box<dyn Matcher>>,
    config: StaticHookConfig,
}

unsafe impl Send for StaticHook {}
unsafe impl Sync for StaticHook {}

impl StaticHook {
    /// Create a new static path with provided configuration
    ///
    /// # Arguments
    ///
    /// * `config` - The configuration for the static path
    ///
    /// # Returns
    ///
    /// * `StaticHook` - The static path
    pub fn new(config: StaticHookConfig) -> StaticHook {
        let matcher = if let Some(matcher_config) = config.matcher() {
            Some(matcher_config.boxed_matcher())
        } else {
            None
        };
        StaticHook { matcher, config }
    }

    async fn read_metadata(&self, file_path: &std::path::Path) -> VetisResult<StaticFile> {
        let path = file_path
            .display()
            .to_string();

        let file = File::open(path.clone()).await;
        let file = match file {
            Ok(file) => {
                let metadata = match file
                    .metadata()
                    .await
                {
                    Ok(metadata) => metadata,
                    Err(e) => {
                        let message =
                            format!("Error getting metadata for file {:?}: {}", file_path, e);
                        return Err(VetisError::Host(HostError::Runtime(
                            RuntimeError::new(&message).with_status_code(StatusCode::NOT_FOUND),
                        )));
                    }
                };

                let modified = metadata
                    .modified()
                    .unwrap_or(std::time::SystemTime::now());

                let file_name = file_path.file_name();

                let mime_type = match file_name {
                    Some(file_name) => match file_name.to_str() {
                        Some(file_name) => match minimime::lookup_by_filename(file_name) {
                            Some(mime) => Some(Str::from_string_buffer(mime.content_type)),
                            None => None,
                        },
                        None => None,
                    },
                    None => None,
                };

                let metadata = StaticFileMetadata {
                    mime: mime_type,
                    #[cfg(unix)]
                    size: metadata.size(),
                    #[cfg(windows)]
                    size: metadata.file_size(),
                    modified,
                    etag: None,
                };

                let static_file = if metadata.size() < 100000 {
                    let file_data = tokio::fs::read(file_path).await;
                    match file_data {
                        Ok(data) => StaticFile::Data { data, metadata },
                        Err(e) => {
                            return Err(VetisError::Host(HostError::Runtime(
                                RuntimeError::new(&e.to_string())
                                    .with_status_code(StatusCode::NOT_FOUND),
                            )));
                        }
                    }
                } else {
                    StaticFile::File { path: file_path.to_path_buf(), metadata }
                };

                Ok(static_file)
            }
            Err(e) => {
                let message = format!("Error opening file {}: {}", path, e);
                Err(VetisError::Host(HostError::Runtime(
                    RuntimeError::new(&message).with_status_code(StatusCode::NOT_FOUND),
                )))
            }
        };

        file
    }

    async fn serve_file(&self, file_path: &PathBuf, range: Option<&str>) -> VetisResult<Response> {
        let file = self
            .read_metadata(file_path)
            .await?;

        let filesize = file
            .metadata()
            .size();

        let mimetype = if let Some(mime) = file
            .metadata()
            .mime()
        {
            HeaderValue::from_bytes(mime.as_bytes())
        } else {
            HeaderValue::from_bytes(b"text/plain")
        }
        .map_err(|e| {
            VetisError::Host(HostError::Runtime(
                RuntimeError::new(&e.to_string())
                    .with_status_code(StatusCode::INTERNAL_SERVER_ERROR),
            ))
        })?;

        if let Some(range) = range {
            let range_info = match range
                .split_once("=")
                .ok_or(VetisError::Host(HostError::Runtime(
                    RuntimeError::new("Missing value").with_status_code(StatusCode::BAD_REQUEST),
                ))) {
                Ok(info) => info,
                Err(e) => return Err(e),
            };

            let (unit, range) = range_info;
            if unit != "bytes" {
                return Err(VetisError::Host(HostError::Runtime(
                    RuntimeError::new("Only bytes ranges are allowed!")
                        .with_status_code(StatusCode::BAD_REQUEST),
                )));
            }

            let (start, end) = range
                .split_once("-")
                .ok_or(VetisError::Host(HostError::Runtime(
                    RuntimeError::new("Invalid format!").with_status_code(StatusCode::BAD_REQUEST),
                )))?;
            let start = start
                .parse::<u64>()
                .map_err(|e| {
                    VetisError::Host(HostError::Runtime(
                        RuntimeError::new(&e.to_string()).with_status_code(StatusCode::BAD_REQUEST),
                    ))
                })?;
            let end = end
                .parse::<u64>()
                .map_err(|e| {
                    VetisError::Host(HostError::Runtime(
                        RuntimeError::new(&e.to_string()).with_status_code(StatusCode::BAD_REQUEST),
                    ))
                })?;
            if start > end || start >= filesize {
                return Ok(Response::builder()
                    .status(http::StatusCode::RANGE_NOT_SATISFIABLE)
                    .body(HttpBody::text("")));
            } else if start < end && end < filesize {
                return Ok(Response::builder()
                    .status(http::StatusCode::PARTIAL_CONTENT)
                    .body(HttpBody::bytes(file.data().unwrap())));
            }
        }

        Ok(Response::builder()
            .status(http::StatusCode::OK)
            .header(
                http::header::ACCEPT_RANGES,
                "bytes"
                    .parse()
                    .unwrap(),
            )
            .header(http::header::CONTENT_LENGTH, HeaderValue::from(filesize))
            .header(http::header::CONTENT_TYPE, mimetype)
            .body(HttpBody::bytes(file.data().unwrap())))
    }

    async fn serve_metadata(&self, file_path: &PathBuf) -> VetisResult<Response> {
        let file = self
            .read_metadata(&file_path)
            .await?;

        let len = file
            .metadata()
            .size();
        let mut headers = HeaderMap::new();
        match len
            .to_string()
            .parse()
        {
            Ok(len) => {
                headers.insert(http::header::CONTENT_LENGTH, len);
            }
            Err(e) => {
                return Err(VetisError::Host(HostError::Runtime(
                    RuntimeError::new(&e.to_string()).with_status_code(StatusCode::BAD_REQUEST),
                )));
            }
        }
        let last_modified = file
            .metadata()
            .modified();
        let date = format_date(last_modified);
        headers.insert(
            http::header::LAST_MODIFIED,
            date.parse()
                .map_err(|e: http::header::InvalidHeaderValue| {
                    VetisError::Host(HostError::Runtime(
                        RuntimeError::new(&e.to_string()).with_status_code(StatusCode::BAD_REQUEST),
                    ))
                })?,
        );

        let mime_type = file
            .metadata()
            .mime();
        if let Some(mime_type) = mime_type {
            headers.insert(
                http::header::CONTENT_TYPE,
                HeaderValue::from_str(mime_type).map_err(|e| {
                    VetisError::Host(HostError::Runtime(
                        RuntimeError::new(&e.to_string()).with_status_code(StatusCode::BAD_REQUEST),
                    ))
                })?,
            );
        }

        let response = Response::builder()
            .status(http::StatusCode::OK)
            .headers(headers)
            .empty();

        Ok(response)
    }
}

impl Hook for StaticHook {
    fn has_matches(&self, context: &Context) -> bool {
        if let Some(matcher) = &self.matcher { matcher.is_match(context) } else { true }
    }

    fn phase(&self) -> &HookPhase {
        &HookPhase::Content
    }

    fn matcher_mut(&mut self) -> Option<&mut Box<dyn Matcher>> {
        self.matcher
            .as_mut()
    }

    fn handle<'a>(
        &'a self,
        request: Request,
        _response: Option<Response>,
    ) -> VetisFutureResult<'a, (Request, Option<Response>)> {
        let future = async move {
            let ext_regex = regex::Regex::new(
                self.config
                    .extensions(),
            );

            let directory = PathBuf::from(
                self.config
                    .directory(),
            );

            let uri = request
                .uri()
                .path()
                .strip_prefix("/")
                .unwrap_or(&request.uri().path());

            let static_dir = if directory.is_relative()
                && let Some(root_dir) = request.root_directory()
            {
                root_dir.join(&directory)
            } else {
                directory.to_path_buf()
            };

            let file = if uri.is_empty() { &static_dir } else { &static_dir.join(uri) };
            if let Some(index_files) = self
                .config
                .index_files()
                && file.is_dir()
            {
                for index in index_files {
                    let path_to_index = static_dir.join(index.as_str());
                    println!("Index file: {:?}", path_to_index);
                    if path_to_index.exists() {
                        let response = Some(
                            self.serve_file(&path_to_index, None)
                                .await?,
                        );

                        return Ok((request, response));
                    }
                }
                println!("No index file found!");
                return Err(VetisError::Host(HostError::Runtime(
                    RuntimeError::new("Access denied.").with_status_code(StatusCode::FORBIDDEN),
                )));
            }

            if !file.exists() {
                return Err(VetisError::Host(HostError::Runtime(
                    RuntimeError::new("File does not exist")
                        .with_status_code(StatusCode::NOT_FOUND),
                )));
            }

            if let Ok(ext_regex) = ext_regex
                && !ext_regex.is_match(uri.as_ref())
            {
                return Err(VetisError::Host(HostError::Runtime(
                    RuntimeError::new("Access denied.").with_status_code(StatusCode::FORBIDDEN),
                )));
            }

            if request.method() == http::Method::HEAD {
                let response = Some(
                    self.serve_metadata(&file)
                        .await?,
                );

                return Ok((request, response));
            }

            let range = if request
                .headers()
                .contains_key(http::header::RANGE)
            {
                if let Some(header) = request
                    .headers()
                    .get(http::header::RANGE)
                {
                    match header.to_str() {
                        Ok(v) => Some(v),
                        Err(_) => None,
                    }
                } else {
                    None
                }
            } else {
                None
            };

            let response = Some(
                self.serve_file(&file, range)
                    .await?,
            );

            Ok((request, response))
        };

        Box::pin(future)
    }
}
