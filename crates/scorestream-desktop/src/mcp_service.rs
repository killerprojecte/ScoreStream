//! In-process MCP lifecycle. Only non-secret, explicitly saved preferences reach disk.
use scorestream_app::Workspace;
use scorestream_mcp::sse::{SseConfig, SseServer};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct McpSettings {
    pub bind: String,
    pub port: u16,
}

impl Default for McpSettings {
    fn default() -> Self {
        Self {
            bind: Ipv4Addr::LOCALHOST.to_string(),
            port: 3210,
        }
    }
}

impl McpSettings {
    fn validate(&self) -> Result<IpAddr, String> {
        let address = self
            .bind
            .parse::<IpAddr>()
            .map_err(|_| "Listen address must be a numeric IPv4 or IPv6 address.".to_string())?;
        if self.port == 0 {
            return Err("Port must be between 1 and 65535.".into());
        }
        Ok(address)
    }
}

#[derive(Clone, Serialize)]
pub struct McpStatus {
    pub settings: McpSettings,
    pub running: bool,
    pub started: bool,
    pub sse_url: Option<String>,
    pub auth_required: bool,
    pub error: Option<String>,
}

pub struct McpService {
    settings: McpSettings,
    settings_path: PathBuf,
    server: Option<SseServer>,
    auth_required: bool,
    error: Option<String>,
}

impl McpService {
    pub fn load(settings_path: PathBuf) -> Self {
        // A saved address never starts a listener. A secret never enters this file.
        let bytes = std::fs::File::open(&settings_path).and_then(|file| {
            let mut bytes = Vec::new();
            file.take(4097).read_to_end(&mut bytes)?;
            Ok(bytes)
        });
        let loaded = match bytes {
            Ok(data) if data.len() <= 4096 => serde_json::from_slice::<McpSettings>(&data)
                .map_err(|_| {
                    "Saved MCP settings are invalid; using localhost defaults.".to_string()
                })
                .and_then(|settings| settings.validate().map(|_| settings)),
            Ok(_) => Err("Saved MCP settings are too large; using localhost defaults.".into()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(McpSettings::default())
            }
            Err(_) => Err("Could not read MCP settings; using localhost defaults.".into()),
        };
        let (settings, error) = match loaded {
            Ok(settings) => (settings, None),
            Err(error) => (McpSettings::default(), Some(error)),
        };
        Self {
            settings,
            settings_path,
            server: None,
            auth_required: false,
            error,
        }
    }

    pub fn status(&self) -> McpStatus {
        let running = self.server.as_ref().is_some_and(SseServer::is_running);
        McpStatus {
            settings: self.settings.clone(),
            running,
            started: self.server.is_some(),
            sse_url: self
                .server
                .as_ref()
                .filter(|_| running)
                .map(|server| format!("http://{}/sse", server.local_addr())),
            auth_required: running && self.auth_required,
            error: if self.server.is_some() && !running {
                Some("MCP service stopped unexpectedly. Stop it before starting again.".into())
            } else {
                self.error.clone()
            },
        }
    }

    pub fn save(&mut self, settings: McpSettings) -> Result<McpStatus, String> {
        if self.server.is_some() {
            return Err("Stop the MCP service before changing its settings.".into());
        }
        let address = settings.validate()?;
        let settings = McpSettings {
            bind: address.to_string(),
            ..settings
        };
        let parent = self
            .settings_path
            .parent()
            .ok_or("Invalid MCP settings path")?;
        std::fs::create_dir_all(parent)
            .map_err(|_| "Could not create the MCP settings directory.")?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)
            .map_err(|_| "Could not create the MCP settings file.")?;
        let bytes = serde_json::to_vec_pretty(&settings).map_err(|e| e.to_string())?;
        temporary
            .write_all(&bytes)
            .and_then(|_| temporary.as_file().sync_all())
            .map_err(|_| "Could not write MCP settings.")?;
        temporary
            .persist(&self.settings_path)
            .map_err(|_| "Could not save MCP settings.")?;
        self.settings = settings;
        self.error = None;
        Ok(self.status())
    }

    pub fn start(
        &mut self,
        settings: McpSettings,
        token: Option<String>,
        workspace: Arc<Mutex<Workspace>>,
    ) -> Result<McpStatus, String> {
        if self.server.is_some() {
            return Err("MCP service is already started. Stop it before changing settings.".into());
        }
        let address = settings.validate()?;
        let token = token.filter(|value| !value.is_empty());
        if token
            .as_ref()
            .is_some_and(|value| value.bytes().any(|b| !(0x21..=0x7e).contains(&b)))
        {
            return Err("The key must use printable ASCII characters without spaces.".into());
        }
        if !address.is_loopback() && token.is_none() {
            return Err("A key is required when listening beyond localhost.".into());
        }
        self.save(settings)?;
        let auth_required = token.is_some();
        match SseServer::start(
            SseConfig {
                bind: address,
                port: self.settings.port,
                token,
            },
            scorestream_mcp::workspace_handler(workspace),
        ) {
            Ok(server) => {
                self.server = Some(server);
                self.auth_required = auth_required;
                self.error = None;
                Ok(self.status())
            }
            Err(error) => {
                let message = format!(
                    "Could not listen on {}:{}: {error}",
                    self.settings.bind, self.settings.port
                );
                self.error = Some(message.clone());
                Err(message)
            }
        }
    }

    pub fn stop(&mut self) -> Result<McpStatus, String> {
        if let Some(mut server) = self.server.take() {
            server.stop().map_err(|error| {
                let message = format!("Could not stop MCP service: {error}");
                self.error = Some(message.clone());
                message
            })?;
        }
        self.auth_required = false;
        self.error = None;
        Ok(self.status())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn service() -> (tempfile::TempDir, McpService) {
        let directory = tempfile::tempdir().unwrap();
        let service = McpService::load(directory.path().join("mcp-service.json"));
        (directory, service)
    }

    #[test]
    fn preferences_round_trip_without_autostart_or_key() {
        let (directory, mut service) = service();
        let settings = McpSettings {
            bind: "::1".into(),
            port: 4321,
        };
        service.save(settings.clone()).unwrap();
        let path = directory.path().join("mcp-service.json");
        let json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(json.as_object().unwrap().len(), 2);
        assert!(json.get("token").is_none());
        let restored = McpService::load(path).status();
        assert_eq!(restored.settings, settings);
        assert!(!restored.running);
        assert!(!restored.auth_required);
        assert!(restored.sse_url.is_none());
    }

    #[test]
    fn invalid_and_old_secret_bearing_files_fall_back_safely() {
        let (directory, _) = service();
        let path = directory.path().join("mcp-service.json");
        for contents in [
            "broken",
            r#"{"bind":"0.0.0.0","port":3210,"token":"old-secret"}"#,
            r#"{"bind":"localhost","port":3210}"#,
            r#"{"bind":"127.0.0.1","port":0}"#,
        ] {
            std::fs::write(&path, contents).unwrap();
            let status = McpService::load(path.clone()).status();
            assert_eq!(status.settings, McpSettings::default());
            assert!(!status.running);
            assert!(status.error.is_some());
        }
    }

    #[test]
    fn validation_rejects_lan_without_key_before_any_bind() {
        let (_, mut service) = service();
        let workspace = Arc::new(Mutex::new(Workspace::demo().unwrap()));
        for bind in ["0.0.0.0", "192.168.1.10", "::", "::ffff:127.0.0.1"] {
            let error = service
                .start(
                    McpSettings {
                        bind: bind.into(),
                        port: 3210,
                    },
                    None,
                    workspace.clone(),
                )
                .err()
                .unwrap();
            assert!(error.contains("key is required"));
        }
        assert!(!service.status().running);
    }

    #[test]
    fn invalid_values_and_header_injection_never_reach_listener() {
        let (_, mut service) = service();
        assert!(
            service
                .save(McpSettings {
                    bind: "localhost".into(),
                    port: 3210
                })
                .is_err()
        );
        assert!(
            service
                .save(McpSettings {
                    bind: "127.0.0.1".into(),
                    port: 0
                })
                .is_err()
        );
        let workspace = Arc::new(Mutex::new(Workspace::demo().unwrap()));
        assert!(
            service
                .start(
                    McpSettings::default(),
                    Some("secret\r\nheader".into()),
                    workspace
                )
                .is_err()
        );
        assert!(service.stop().is_ok());
        assert!(service.stop().is_ok());
    }

    #[test]
    fn loopback_start_stop_retry_and_restart_keep_secret_out_of_status_and_disk() {
        let (directory, mut service) = service();
        let occupied = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = occupied.local_addr().unwrap().port();
        let settings = McpSettings {
            bind: "127.0.0.1".into(),
            port,
        };
        let workspace = Arc::new(Mutex::new(Workspace::demo().unwrap()));
        let secret = "unit-test-session-secret";
        assert!(
            service
                .start(settings.clone(), Some(secret.into()), workspace.clone())
                .is_err()
        );
        assert!(!service.status().running);
        assert!(
            service
                .status()
                .error
                .as_ref()
                .unwrap()
                .contains("Could not listen")
        );
        drop(occupied);
        let started = service
            .start(settings.clone(), Some(secret.into()), workspace.clone())
            .unwrap();
        assert!(started.running && started.auth_required);
        assert_eq!(
            started.sse_url.as_deref(),
            Some(format!("http://127.0.0.1:{port}/sse").as_str())
        );
        assert!(!serde_json::to_string(&started).unwrap().contains(secret));
        assert!(
            !std::fs::read_to_string(directory.path().join("mcp-service.json"))
                .unwrap()
                .contains(secret)
        );
        assert!(
            service
                .start(settings.clone(), None, workspace.clone())
                .is_err()
        );
        assert!(service.save(settings.clone()).is_err());
        assert!(!service.stop().unwrap().running);
        let restarted = service.start(settings, None, workspace).unwrap();
        assert!(restarted.running && !restarted.auth_required);
        service.stop().unwrap();
        assert!(std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, port)).is_ok());
    }
}
