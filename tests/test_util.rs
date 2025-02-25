use std::net::{Ipv4Addr, SocketAddrV4, TcpListener};

use bindle::client::{tokens::NoToken, Client};

pub struct TestController {
    pub client: Client<NoToken>,
    pub base_url: String,
    server_handle: std::process::Child,
    // Keep a handle to the tempdir so it doesn't drop until the controller drops
    _tempdir: tempfile::TempDir,
}

impl TestController {
    /// Builds a new test controller, using the given binary name to start the server (e.g. if your
    /// project is called bindle-foo, then `bindle-foo` would be the argument to this function).
    /// Waits for up to 10 seconds for the server to run
    pub async fn new(server_binary_name: &str) -> TestController {
        let build_result = tokio::task::spawn_blocking(|| {
            std::process::Command::new("cargo")
                .args(&["build", "--features", "cli"])
                .output()
        })
        .await
        .unwrap()
        .expect("unable to run build command");

        assert!(
            build_result.status.success(),
            "Error trying to build server {}",
            String::from_utf8(build_result.stderr).unwrap()
        );

        let tempdir = tempfile::tempdir().expect("unable to create tempdir");

        let address = format!("127.0.0.1:{}", get_random_port());

        let base_url = format!("http://{}/v1/", address);

        let server_handle = std::process::Command::new(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("target/debug")
                .join(server_binary_name),
        )
        .args(&[
            "--unauthenticated",
            "-d",
            tempdir.path().to_string_lossy().to_string().as_str(),
            "-i",
            address.as_str(),
        ])
        .spawn()
        .expect("unable to start bindle server");

        // Wait until we can connect to the server so we know it is available
        let mut wait_count = 1;
        loop {
            // Magic number: 10 + 1, since we are starting at 1 for humans
            if wait_count >= 11 {
                panic!("Ran out of retries waiting for server to start");
            }
            match tokio::net::TcpStream::connect(&address).await {
                Ok(_) => break,
                Err(e) => {
                    eprintln!("Waiting for server to come up, attempt {}. Will retry in 1 second. Got error {:?}", wait_count, e);
                    wait_count += 1;
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }
        }

        let client = Client::new(&base_url, NoToken).expect("unable to setup bindle client");
        TestController {
            client,
            base_url,
            server_handle,
            _tempdir: tempdir,
        }
    }
}

impl Drop for TestController {
    fn drop(&mut self) {
        // Not much we can do here if we error, so just ignore
        let _ = self.server_handle.kill();
    }
}

fn get_random_port() -> u16 {
    TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
        .expect("Unable to bind to check for port")
        .local_addr()
        .unwrap()
        .port()
}
