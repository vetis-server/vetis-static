#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
use crate::tokio::StaticHook;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, time::SystemTime};
use time::{OffsetDateTime, format_description::well_known::Rfc2822};
use vetis::{
    Str, VetisHook, VetisResult,
    errors::{ConfigError, VetisError},
    hook::HookConfig,
    matcher::{self, MatcherConfig},
};

#[cfg(feature = "runtime-tokio")]
/// Tokio runtime support module
pub mod tokio;

#[cfg(test)]
mod tests;

/// Format a SystemTime to a string in RFC 2822 format.
///
/// # Arguments
///
/// * `date` - The SystemTime to format.
///
/// # Returns
///
/// * `String` - The formatted date.
pub fn format_date(date: SystemTime) -> String {
    let date = OffsetDateTime::from(date);
    date.format(&Rfc2822)
        .unwrap()
}

/// Builder for creating `StaticHookConfig` instances.
pub struct StaticHookConfigBuilder {
    matcher: Option<Box<dyn matcher::MatcherConfig>>,
    extensions: Str,
    directory: Str,
    index_files: Option<Vec<Str>>,
}

impl StaticHookConfigBuilder {
    /// Add a Matcher to hook
    ///
    /// # Arguments
    ///
    /// * `matcher` - Matcher configuration
    ///
    /// # Notes
    ///
    /// Add a Matcher configuration to the hook, matchers are
    /// useful to add determine if a particular hook will
    /// be used.
    pub fn matcher<M>(mut self, matcher: M) -> Self
    where
        M: MatcherConfig + 'static,
    {
        self.matcher = Some(Box::new(matcher));
        self
    }

    /// Allow set the extensions of the static path.
    ///
    /// # Returns
    ///
    /// * `Self` - The builder.
    pub fn extensions(mut self, extensions: &str) -> Self {
        self.extensions = extensions.into();
        self
    }

    /// Allow set the directory of the static path.
    ///
    /// # Returns
    ///
    /// * `Self` - The builder.
    pub fn directory(mut self, directory: &str) -> Self {
        self.directory = directory.into();
        self
    }

    /// Allow set the index files of the static path.
    ///
    /// # Returns
    ///
    /// * `Self` - The builder.
    pub fn index_files(mut self, index_files: Vec<Str>) -> Self {
        self.index_files = Some(index_files);
        self
    }

    /// Build the `StaticHookConfig` with the configured settings.
    ///
    /// # Returns
    ///
    /// * `VetisResult<StaticHookConfig>` - The `StaticHookConfig` with the configured settings.
    pub fn build(self) -> VetisResult<StaticHookConfig> {
        if self
            .extensions
            .is_empty()
        {
            return Err(VetisError::Config(ConfigError::Hook("Extensions cannot be empty".into())));
        }
        if self
            .directory
            .is_empty()
        {
            return Err(VetisError::Config(ConfigError::Hook("Directory cannot be empty".into())));
        }

        Ok(StaticHookConfig {
            matcher: self
                .matcher
                .map_or(None, |val| Some(val)),
            extensions: self.extensions,
            directory: self.directory,
            index_files: self
                .index_files
                .map_or(None, |val| Some(val.into_boxed_slice())),
        })
    }
}

/// Configuration for static file serving.
#[derive(Clone, Serialize, Deserialize)]
pub struct StaticHookConfig {
    matcher: Option<Box<dyn MatcherConfig>>,
    extensions: Str,
    directory: Str,
    index_files: Option<Box<[Str]>>,
}

#[typetag::serde(name = "static")]
impl HookConfig for StaticHookConfig {
    fn matcher(&self) -> Option<&Box<dyn MatcherConfig>> {
        self.matcher
            .as_ref()
    }

    fn boxed_clone(&self) -> Box<dyn HookConfig> {
        Box::new(self.clone())
    }

    fn boxed_hook(&self) -> Box<dyn VetisHook> {
        Box::new(StaticHook::new(self.clone()))
    }
}

impl StaticHookConfig {
    /// Allow create a new `StaticHookConfigBuilder` with default settings.
    ///
    /// # Returns
    ///
    /// * `StaticHookConfigBuilder` - The builder.
    pub fn builder() -> StaticHookConfigBuilder {
        StaticHookConfigBuilder {
            matcher: None,
            extensions: ".html".into(),
            directory: ".".into(),
            index_files: None,
        }
    }

    /// Returns extensions
    ///
    /// # Returns
    ///
    /// * `&str` - The extensions.
    pub fn extensions(&self) -> &str {
        &self.extensions
    }

    /// Returns directory
    ///
    /// # Returns
    ///
    /// * `&str` - The directory.
    pub fn directory(&self) -> &str {
        &self.directory
    }

    /// Returns index_files
    ///
    /// # Returns
    ///
    /// * `&Option<Vec<String>>` - The index_files.
    pub fn index_files(&self) -> Option<&Box<[Str]>> {
        self.index_files
            .as_ref()
    }
}

/// Builder for creating `StaticFileMetadata` instances.
#[derive(Debug, Clone)]
pub struct StaticFileMetadataBuilder {
    mime: Option<Str>,
    size: u64,
    modified: std::time::SystemTime,
    etag: Option<Str>,
}

impl StaticFileMetadataBuilder {
    /// Sets the mime type.
    ///
    /// # Arguments
    ///
    /// * `mime` - The mime type.
    ///
    /// # Returns
    ///
    /// * `Self` - The builder.
    pub fn mime(mut self, mime: Str) -> Self {
        self.mime = Some(mime);
        self
    }

    /// Sets the size.
    ///
    /// # Arguments
    ///
    /// * `size` - The size.
    ///
    /// # Returns
    ///
    /// * `Self` - The builder.
    pub fn size(mut self, size: u64) -> Self {
        self.size = size;
        self
    }

    /// Sets the modified time.
    ///
    /// # Arguments
    ///
    /// * `modified` - The modified time.
    ///
    /// # Returns
    ///
    /// * `Self` - The builder.
    pub fn modified(mut self, modified: std::time::SystemTime) -> Self {
        self.modified = modified;
        self
    }

    /// Sets the etag.
    ///
    /// # Arguments
    ///
    /// * `etag` - The etag.
    ///
    /// # Returns
    ///
    /// * `Self` - The builder.
    pub fn etag(mut self, etag: Str) -> Self {
        self.etag = Some(etag);
        self
    }

    /// Builds the `StaticFileMetadata`.
    ///
    /// # Returns
    ///
    /// * `StaticFileMetadata` - The built metadata.
    pub fn build(self) -> StaticFileMetadata {
        StaticFileMetadata {
            mime: self.mime,
            size: self.size,
            modified: self.modified,
            etag: self.etag,
        }
    }
}

#[derive(Clone, Debug)]
/// Metadata for a static file.
pub struct StaticFileMetadata {
    mime: Option<Str>,
    size: u64,
    modified: std::time::SystemTime,
    etag: Option<Str>,
}

impl StaticFileMetadata {
    /// Creates a new builder.
    ///
    /// # Returns
    ///
    /// * `StaticFileMetadataBuilder` - The new builder.
    pub fn builder() -> StaticFileMetadataBuilder {
        StaticFileMetadataBuilder {
            mime: None,
            size: 0,
            modified: std::time::SystemTime::now(),
            etag: None,
        }
    }

    /// Returns the mime type.
    ///
    /// # Returns
    ///
    /// * `Option<&Str>` - The mime type.
    pub fn mime(&self) -> Option<&Str> {
        self.mime.as_ref()
    }

    /// Returns the size.
    ///
    /// # Returns
    ///
    /// * `u64` - The size.
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Returns the modified time.
    ///
    /// # Returns
    ///
    /// * `std::time::SystemTime` - The modified time.
    pub fn modified(&self) -> std::time::SystemTime {
        self.modified
    }

    /// Returns the etag.
    ///
    /// # Returns
    ///
    /// * `Option<&Str>` - The etag.
    pub fn etag(&self) -> Option<&Str> {
        self.etag.as_ref()
    }
}

#[derive(Clone)]
/// A static file that can be either in memory or on disk.
pub enum StaticFile {
    /// Data stored in memory.
    Data {
        /// The data.
        data: Vec<u8>,
        /// The metadata.
        metadata: StaticFileMetadata,
    },
    /// Data stored on disk.
    File {
        /// The path to the file.
        path: PathBuf,
        /// The metadata.
        metadata: StaticFileMetadata,
    },
}

impl StaticFile {
    /// Returns the metadata.
    ///
    /// # Returns
    ///
    /// * `&StaticFileMetadata` - The metadata.
    pub fn metadata(&self) -> &StaticFileMetadata {
        match self {
            StaticFile::Data { metadata, .. } => metadata,
            StaticFile::File { metadata, .. } => metadata,
        }
    }

    /// Returns the data if it is in memory.
    ///
    /// # Returns
    ///
    /// * `Option<&Vec<u8>>` - The data if it is in memory.
    pub fn data(&self) -> Option<&Vec<u8>> {
        match self {
            StaticFile::Data { data, .. } => Some(data),
            StaticFile::File { .. } => None,
        }
    }

    /// Returns the path if it is on disk.
    ///
    /// # Returns
    ///
    /// * `Option<&PathBuf>` - The path if it is on disk.
    pub fn path(&self) -> Option<&PathBuf> {
        match self {
            StaticFile::Data { .. } => None,
            StaticFile::File { path, .. } => Some(path),
        }
    }
}
