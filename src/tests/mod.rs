use crate::StaticHookConfig;
use vetis::Str;
use vetis::{
    VetisTestResult,
    errors::{ConfigError, VetisError},
};
use vetis_matchers::request::PathMatcherConfig;

#[test]
fn test_static_path_config() -> VetisTestResult<()> {
    let path_config = StaticHookConfig::builder()
        .matcher(
            PathMatcherConfig::builder()
                .add_path("/test/*")
                .build()?,
        )
        .extensions(".html")
        .directory("./test")
        .build()?;

    assert_eq!(path_config.directory(), "./test");
    assert_eq!(path_config.extensions(), ".html");

    Ok(())
}

#[test]
fn test_invalid_extensions() -> VetisTestResult<()> {
    let some_path = StaticHookConfig::builder()
        .matcher(
            PathMatcherConfig::builder()
                .add_path("/test/*")
                .build()?,
        )
        .extensions("")
        .build();

    assert!(some_path.is_err());
    assert_eq!(
        some_path.err(),
        Some(VetisError::Config(ConfigError::Hook("Extensions cannot be empty".into(),)))
    );

    Ok(())
}

#[test]
fn test_invalid_directory() -> VetisTestResult<()> {
    let some_path = StaticHookConfig::builder()
        .matcher(
            PathMatcherConfig::builder()
                .add_path("/test/*")
                .build()?,
        )
        .extensions(".html")
        .directory("")
        .build();

    assert!(some_path.is_err());
    assert_eq!(
        some_path.err(),
        Some(VetisError::Config(ConfigError::Hook("Directory cannot be empty".into(),)))
    );

    Ok(())
}

#[test]
fn test_static_files_config() -> VetisTestResult<()> {
    let static_files_config = StaticHookConfig::builder()
        .extensions("html,css,js")
        .directory("/var/vetis/www")
        .index_files(vec!["index.html".into(), "index.htm".into()])
        .build()?;
    assert_eq!(static_files_config.extensions(), "html,css,js");
    assert_eq!(static_files_config.directory(), "/var/vetis/www");
    assert_eq!(
        static_files_config.index_files(),
        Some(Box::new([Str::const_new("index.html"), Str::const_new("index.htm")]) as Box<[Str]>)
            .as_ref()
    );
    Ok(())
}
