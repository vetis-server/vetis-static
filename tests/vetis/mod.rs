#[cfg(feature = "runtime-tokio")]
mod tokio {
    use crate::common::{CA_CERT, SERVER_CERT, SERVER_KEY};
    use caramelo::{
        expect,
        matchers::{eq, truthy},
    };
    use deboa::{
        cert::{CertificateExt as _, ContentEncoding},
        request,
    };
    use deboa_tokio::cert::DeboaCertificate;
    use http::{StatusCode, Version};
    use serde::Deserialize;
    use std::{
        net::{IpAddr, Ipv4Addr},
        path::Path,
    };
    use tokio::fs::read_to_string;
    use vetis::{VetisServer, VetisTestResult, host::HostConfig, security::TlsConfig};
    #[allow(unused_imports)]
    use vetis_log::StderrLogConfig;
    use vetis_macros::status_pages;
    use vetis_static::{StaticHookConfig, tokio::StaticHook};
    use vetis_tokio::{ServerConfig, Vetis, host::Host};

    #[allow(unused)]
    #[derive(Deserialize, Clone)]
    pub struct VetisConfig {
        worker_threads: usize,
        max_blocking_threads: usize,
        server: ServerConfig,
    }

    #[tokio::test]
    async fn test_from_file() -> VetisTestResult<()> {
        let config_path = Path::new("files/public/vetis-static.yaml");
        expect(config_path.exists()).to_be(truthy());

        let file = read_to_string(&config_path)
            .await
            .unwrap();
        let config = serde_yaml_ng::from_str::<VetisConfig>(&file).unwrap();
        let config_clone = config
            .server
            .clone();
        expect(
            config_clone
                .hosts()
                .len(),
        )
        .to_be(eq(1));

        let host = &config_clone.hosts()[0];
        expect(host.hostname()).to_be(eq("localhost"));
        expect(
            host.hooks()
                .is_some(),
        )
        .to_be(truthy());

        let mut server = Vetis::from_config(config.server).await?;
        server
            .start()
            .await?;

        let cert = tokio::fs::read(CA_CERT).await?;
        let client = deboa_tokio::Client::builder()
            .certificate(DeboaCertificate::from_slice(&cert, ContentEncoding::DER))
            .build();

        let request = request::get("https://localhost:8443/")?
            .version(Version::HTTP_2)
            .send_with(&client)
            .await?;

        assert_eq!(request.status(), http::StatusCode::OK);

        server
            .stop()
            .await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_index() -> VetisTestResult<()> {
        let tls_config = TlsConfig::builder()
            .ca_file(CA_CERT)
            .cert_file(SERVER_CERT)
            .key_file(SERVER_KEY)
            .build()?;

        let host_config = HostConfig::builder()
            .hostname("localhost")
            .protos(&[Version::HTTP_11])
            .root_directory(".")
            .tls(tls_config)
            .bind_addresses(&[(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 9100)])
            .build()?;

        let mut virtual_host = Host::new(host_config).await?;
        virtual_host.add_hook(StaticHook::new(
            StaticHookConfig::builder()
                .directory("files/public")
                .index_files(vec!["index.html".into()])
                .build()?,
        ));

        let mut server = Vetis::builder()
            .add_host(virtual_host)
            .await?
            .build();

        server
            .start()
            .await?;

        let cert = tokio::fs::read(CA_CERT).await?;
        let client = deboa_tokio::Client::builder()
            .certificate(DeboaCertificate::from_slice(&cert, ContentEncoding::DER))
            .build();

        let request = request::get("https://localhost:9100/")?
            .version(Version::HTTP_11)
            .send_with(&client)
            .await?;

        assert_eq!(request.status(), http::StatusCode::OK);

        let expected = if cfg!(windows) {
            "<html>\r\n<head>\r\n  <title>\r\n    Tested!\r\n  </title>\r\n</head>\r\n<body>\r\n  <p>\r\n    Tested!\r\n  </p>\r\n</body>\r\n</html>\r\n"
        } else {
            "<html>\n<head>\n  <title>\n    Tested!\n  </title>\n</head>\n<body>\n  <p>\n    Tested!\n  </p>\n</body>\n</html>\n"
        };

        expect(
            request
                .text()
                .await?,
        )
        .to_be(eq(expected.to_string()));

        server
            .stop()
            .await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_not_found() -> VetisTestResult<()> {
        let tls_config = TlsConfig::builder()
            .ca_file(CA_CERT)
            .cert_file(SERVER_CERT)
            .key_file(SERVER_KEY)
            .build()?;

        let host_config = HostConfig::builder()
            .hostname("localhost")
            .protos(&[Version::HTTP_11])
            .root_directory(".")
            .tls(tls_config)
            .bind_addresses(&[(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 9000)])
            .status_pages(status_pages! {
               404 @ "files/public/404.html"
            })
            .build()?;

        let mut virtual_host = Host::new(host_config).await?;
        virtual_host.add_hook(StaticHook::new(
            StaticHookConfig::builder()
                .directory("files/public")
                .build()?,
        ));

        let mut server = Vetis::builder()
            .add_host(virtual_host)
            .await?
            .build();

        server
            .start()
            .await?;

        let cert = tokio::fs::read(CA_CERT).await?;
        let client = deboa_tokio::Client::builder()
            .certificate(DeboaCertificate::from_slice(&cert, ContentEncoding::DER))
            .build();

        let request = request::get("https://localhost:9000/some/file/here.txt")?
            .version(Version::HTTP_11)
            .send_with(&client)
            .await?;

        expect(request.status()).to_be(eq(StatusCode::NOT_FOUND));

        server
            .stop()
            .await?;

        Ok(())
    }
}
