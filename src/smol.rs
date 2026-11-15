use crate::{format_date, StaticFile, StaticFileMetadata, StaticHookConfig};
use http::{HeaderMap, HeaderValue};
use hyper_body_utils::HttpBody;
use log::error;
use smol::fs::File;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
#[cfg(windows)]
use std::os::windows::fs::MetadataExt;
use std::{future::Future, path::PathBuf, pin::Pin, sync::Arc};
use vetis::{
    errors::{FileError, HostError, VetisError},
    virtual_host::path::Hook,
    Request, Response, VetisResult,
};

/// Static path
pub struct StaticHook {
    config: StaticHookConfig,
    index_file: Option<String>,
}

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
        if let Some(index_files) = config.index_files() {
            let directory = PathBuf::from(config.directory());
            if let Some(index_file) = index_files
                .iter()
                .find(|index_file| {
                    directory
                        .join(index_file)
                        .exists()
                })
            {
                return StaticHook {
                    config: config.clone(),
                    index_file: Some(index_file.to_string()),
                };
            }
        }
        StaticHook { config, index_file: None }
    }

    async fn cache_file(&self, file_path: &std::path::Hook) -> VetisResult<StaticFile> {
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
                        error!("Error getting metadata for file {:?}: {}", file_path, e);
                        return Err(VetisError::Host(HostError::File(FileError::NotFound)));
                    }
                };

                let modified = metadata
                    .modified()
                    .unwrap_or(std::time::SystemTime::now());

                let file_name = file_path.file_name();

                let mime_type = match file_name {
                    Some(file_name) => match file_name.to_str() {
                        Some(file_name) => match minimime::lookup_by_filename(file_name) {
                            Some(mime) => Some(mime.content_type),
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

                let max_file_size = if let Some(cache) = self.config.cache() {
                    cache.max_file_size() as u64
                } else {
                    1024 * 1024 * 10 // 10MB default
                };

                let static_file = if metadata.size() < max_file_size {
                    let data = smol::fs::read(file_path).await;
                    if let Ok(data) = data {
                        StaticFile::Data { data, metadata }
                    } else {
                        return Err(VetisError::Host(HostError::File(FileError::NotFound)));
                    }
                } else {
                    StaticFile::File { path: file_path.to_path_buf(), metadata }
                };

                Ok(static_file)
            }
            Err(e) => {
                error!("Error opening file {}: {}", path, e);
                Err(VetisError::Host(HostError::File(FileError::NotFound)))
            }
        };

        file
    }

    async fn serve_file(
        &self,
        file_path: &std::path::Hook,
        range: Option<&str>,
    ) -> VetisResult<Response> {
        let file = self
            .cache_file(file_path)
            .await?;

        let filesize = file
            .metadata()
            .size();

        if let Some(range) = range {
            let range_info = match range
                .split_once("=")
                .ok_or(VetisError::Host(HostError::File(FileError::InvalidRange)))
            {
                Ok(info) => info,
                Err(e) => return Err(e),
            };

            let (unit, range) = range_info;
            if unit != "bytes" {
                return Err(VetisError::Host(HostError::File(FileError::InvalidRange)));
            }

            let (start, end) = range
                .split_once("-")
                .ok_or(VetisError::Host(HostError::File(FileError::InvalidRange)))?;
            let start = start
                .parse::<u64>()
                .map_err(|_| VetisError::Host(HostError::File(FileError::InvalidRange)))?;
            let end = end
                .parse::<u64>()
                .map_err(|_| VetisError::Host(HostError::File(FileError::InvalidRange)))?;
            if start > end || start >= filesize {
                return Ok(Response::builder()
                    .status(http::StatusCode::RANGE_NOT_SATISFIABLE)
                    .body(HttpBody::empty()));
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
            .body(HttpBody::bytes(file.data().unwrap())))
    }

    async fn serve_metadata(&self, file_path: PathBuf) -> VetisResult<Response> {
        let file = self
            .cache_file(&file_path)
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
            Err(_) => todo!(),
        }
        let last_modified = file
            .metadata()
            .modified();
        let date = format_date(last_modified);
        headers.insert(
            http::header::LAST_MODIFIED,
            date.parse()
                .map_err(|_| VetisError::Host(HostError::File(FileError::InvalidMetadata)))?,
        );

        let mime_type = file
            .metadata()
            .mime();
        if let Some(mime_type) = mime_type {
            headers.insert(
                http::header::CONTENT_TYPE,
                HeaderValue::from_str(mime_type)
                    .map_err(|_| VetisError::Host(HostError::File(FileError::InvalidMetadata)))?,
            );
        }

        let response = Response::builder()
            .status(http::StatusCode::OK)
            .headers(headers)
            .text("");

        Ok(response)
    }

    async fn serve_index_file(&self, directory: &std::path::Hook) -> Result<Response, VetisError> {
        match &self.index_file {
            Some(index_file) => {
                let full_path = directory.join(index_file);
                self.serve_file(&full_path, None)
                    .await
            }
            None => {
                println!("No index file configured");
                Err(VetisError::Host(HostError::File(FileError::NotFound)))
            }
        }
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
        mut response: Option<Response>,
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

            let file = static_dir.join(uri);
            if let Some(index_files) = self
                .config
                .index_files()
                && file.is_dir()
            {
                for index in index_files {
                    let path_to_index = static_dir.join(index.as_str());
                    if path_to_index.exists() {
                        response = Some(
                            self.serve_file(&path_to_index, None)
                                .await?,
                        );
                    }
                }
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
                response = Some(
                    self.serve_metadata(&file)
                        .await?,
                );
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

            response = Some(
                self.serve_file(&file, range)
                    .await?,
            );

            Ok((request, response))
        };

        Box::pin(future)
    }
}
