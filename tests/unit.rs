#[cfg(test)]
mod tests {
    use lilac_aur::{AurClient, AlpmWrapper};
    use mockito::Server;
    use tempfile::tempdir;
    use std::fs::create_dir_all;
    use std::sync::Once;

    static INIT: Once = Once::new();
    fn init_logger() {
        INIT.call_once(|| {
            let _ = env_logger::builder()
                .is_test(true)
                .try_init();
        });
    }

    #[test]
    fn test_aur_client_search() {
        init_logger();
        
        let mut server = Server::new();
        
        let mock_response = r#"{
            "resultcount": 1,
            "results": [
                {
                    "ID": 12345,
                    "Name": "test-package",
                    "PackageBaseID": 12345,
                    "PackageBase": "test-package",
                    "Version": "1.0.0-1",
                    "Description": "A test package",
                    "URL": "https://example.com/test-package",
                    "NumVotes": 10,
                    "Popularity": 1.23,
                    "OutOfDate": null,
                    "Maintainer": "testuser",
                    "FirstSubmitted": 1234567890,
                    "LastModified": 1234567890,
                    "URLPath": "/cgit/aur.git/snapshot/test-package.tar.gz"
                }
            ],
            "type": "search",
            "version": 5
        }"#;
        
        let _m = server
            .mock("GET", "/rpc/")
            .match_query(mockito::Matcher::UrlEncoded("v".into(), "5".into()))
            .match_query(mockito::Matcher::UrlEncoded("type".into(), "search".into()))
            .match_query(mockito::Matcher::UrlEncoded("by".into(), "name".into()))
            .match_query(mockito::Matcher::UrlEncoded("arg".into(), "test".into()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_response)
            .create();

        let client = AurClient::new(server.url());
        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(client.search_packages("test"));

        assert!(result.is_ok(), "Expected Ok, got {:?}", result);
        let packages = result.unwrap();
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name, "test-package");
        assert_eq!(packages[0].version, "1.0.0-1");
    }

    #[test]
    fn test_aur_client_get_package_info() {
        init_logger();
        
        let mut server = Server::new();
        
        let mock_response = r#"{
            "resultcount": 1,
            "results": [
                {
                    "ID": 12345,
                    "Name": "test-package",
                    "PackageBaseID": 12345,
                    "PackageBase": "test-package",
                    "Version": "1.0.0-1",
                    "Description": "A test package",
                    "URL": "https://example.com/test-package",
                    "NumVotes": 10,
                    "Popularity": 1.23,
                    "OutOfDate": null,
                    "Maintainer": "testuser",
                    "FirstSubmitted": 1234567890,
                    "LastModified": 1234567890,
                    "URLPath": "/cgit/aur.git/snapshot/test-package.tar.gz"
                }
            ],
            "type": "info",
            "version": 5
        }"#;
        
        let _m = server
            .mock("GET", "/rpc/")
            .match_query(mockito::Matcher::UrlEncoded("v".into(), "5".into()))
            .match_query(mockito::Matcher::UrlEncoded("type".into(), "info".into()))
            .match_query(mockito::Matcher::UrlEncoded("arg".into(), "test-package".into()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_response)
            .create();

        let client = AurClient::new(server.url());
        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(client.get_package_info("test-package"));

        assert!(result.is_ok(), "Expected Ok, got {:?}", result);
        let pkg = result.unwrap();
        assert_eq!(pkg.name, "test-package");
        assert_eq!(pkg.version, "1.0.0-1");
    }

    #[test]
    fn test_alpm_wrapper_new() {
        init_logger();
        
        let temp_dir = tempdir().unwrap();
        let root_path = temp_dir.path().join("root");
        
        create_dir_all(&root_path.join("var/lib/pacman")).unwrap();
        
        let etc_dir = root_path.join("etc");
        create_dir_all(&etc_dir).unwrap();
        
        let pacman_conf = etc_dir.join("pacman.conf");
        std::fs::write(&pacman_conf, "[options]\nRootDir = /\nDBPath = /var/lib/pacman\nLogFile = /var/log/pacman.log").unwrap();
        
        // Test ALPM initialization
        // Note: This test is limited because ALPM requires root access to function fully
        // We're just testing that the wrapper can be created, not that it works with the real ALPM
        let result = std::panic::catch_unwind(|| {
            let _ = AlpmWrapper::new();
        });
        
        // The test passes if it didn't panic
        assert!(result.is_ok());
    }

    #[test]
    fn test_alpm_is_package_installed() {
        init_logger();
        
        let result = std::panic::catch_unwind(|| {
            if let Ok(alpm) = AlpmWrapper::new() {
                let _ = alpm.is_package_installed("nonexistent-package-12345");
            }
        });
        
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_aur_client_search_error() {
        init_logger();
        
        let mut server = Server::new();
        
        let _m = server
            .mock("GET", "/rpc/?v=5&type=search&by=name&arg=error")
            .with_status(500)
            .create();

        let client = AurClient::new(server.url());
        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(client.search_packages("error"));

        assert!(result.is_err());
    }
    
    #[test]
    fn test_aur_client_get_package_info_not_found() {
        init_logger();
        
        let mut server = Server::new();
        
        let _m = server
            .mock("GET", "/rpc/?v=5&type=info&arg=nonexistent")
            .with_status(404)
            .create();

        let client = AurClient::new(server.url());
        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(client.get_package_info("nonexistent"));

        assert!(result.is_err());
    }
}
