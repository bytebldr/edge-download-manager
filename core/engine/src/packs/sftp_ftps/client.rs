//! SFTP & FTPS Transfer Client
//! Parses `sftp://` and `ftps://` URLs into validated `RemoteTransferSpec`s and builds
//! injection-safe subprocess plans (`sftp.exe` / `curl.exe`) with `CREATE_NO_WINDOW`.

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Stdio;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RemoteProtocol {
    Sftp,
    Ftps,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteTransferSpec {
    pub protocol: RemoteProtocol,
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub remote_path: String,
    pub strict_host_key: bool,
    pub verify_tls: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferPlan {
    pub program: String,
    pub args: Vec<String>,
}

/// Parse an `sftp://` or `ftps://` URL into a validated `RemoteTransferSpec`.
pub fn parse_sftp_or_ftps_url(url_str: &str) -> Result<RemoteTransferSpec, String> {
    let parsed = url::Url::parse(url_str).map_err(|e| format!("Invalid remote URL: {}", e))?;

    let (protocol, default_port) = match parsed.scheme() {
        "sftp" => (RemoteProtocol::Sftp, 22),
        "ftps" => (RemoteProtocol::Ftps, 990),
        other => {
            return Err(format!(
                "Unsupported scheme '{}'; expected 'sftp' or 'ftps'",
                other
            ))
        }
    };

    let host = parsed
        .host_str()
        .map(str::trim)
        .filter(|h| !h.is_empty())
        .ok_or_else(|| "Remote URL missing host".to_string())?
        .to_string();
    reject_control_chars(&host, "host")?;

    let port = parsed.port().unwrap_or(default_port);

    let username = if parsed.username().is_empty() {
        None
    } else {
        let u = parsed.username().to_string();
        reject_control_chars(&u, "username")?;
        Some(u)
    };

    let password = match parsed.password() {
        Some(p) => {
            reject_control_chars(p, "password")?;
            Some(p.to_string())
        }
        None => None,
    };

    let remote_path = parsed.path().to_string();
    if remote_path.is_empty() || remote_path == "/" {
        return Err("Remote URL missing file path".to_string());
    }
    reject_control_chars(&remote_path, "remote_path")?;

    Ok(RemoteTransferSpec {
        protocol,
        host,
        port,
        username,
        password,
        remote_path,
        strict_host_key: true,
        verify_tls: true,
    })
}

/// Build an argument-array subprocess plan (never via `cmd /c` or shell interpolation).
pub fn build_transfer_plan(
    spec: &RemoteTransferSpec,
    local_dest: &Path,
) -> Result<TransferPlan, String> {
    reject_control_chars(&spec.host, "host")?;
    reject_control_chars(&spec.remote_path, "remote_path")?;
    if let Some(u) = &spec.username {
        reject_control_chars(u, "username")?;
    }
    if let Some(p) = &spec.password {
        reject_control_chars(p, "password")?;
    }

    let dest_str = local_dest
        .to_str()
        .ok_or_else(|| "Destination path is not valid UTF-8".to_string())?;
    if dest_str.is_empty() {
        return Err("Destination path cannot be empty".to_string());
    }
    reject_control_chars(dest_str, "local_dest")?;

    match spec.protocol {
        RemoteProtocol::Ftps => {
            let program = if cfg!(windows) { "curl.exe" } else { "curl" }.to_string();
            let mut args = vec![
                "--fail".to_string(),
                "--silent".to_string(),
                "--show-error".to_string(),
                "--location".to_string(),
                "--ssl-reqd".to_string(),
            ];
            if !spec.verify_tls {
                args.push("--insecure".to_string());
            }
            if spec.username.is_some() || spec.password.is_some() {
                args.push("--user".to_string());
                args.push(format!(
                    "{}:{}",
                    spec.username.as_deref().unwrap_or("anonymous"),
                    spec.password.as_deref().unwrap_or("")
                ));
            }
            args.push("-o".to_string());
            args.push(dest_str.to_string());
            args.push(format!(
                "ftps://{}:{}{}",
                spec.host, spec.port, spec.remote_path
            ));
            Ok(TransferPlan { program, args })
        }
        RemoteProtocol::Sftp => {
            let program = if cfg!(windows) { "sftp.exe" } else { "sftp" }.to_string();
            let host_key_opt = if spec.strict_host_key {
                "StrictHostKeyChecking=yes"
            } else {
                "StrictHostKeyChecking=accept-new"
            };
            let source = match &spec.username {
                Some(u) => format!("{}@{}:{}", u, spec.host, spec.remote_path),
                None => format!("{}:{}", spec.host, spec.remote_path),
            };
            let args = vec![
                "-P".to_string(),
                spec.port.to_string(),
                "-o".to_string(),
                "BatchMode=yes".to_string(),
                "-o".to_string(),
                host_key_opt.to_string(),
                source,
                dest_str.to_string(),
            ];
            Ok(TransferPlan { program, args })
        }
    }
}

/// Execute a remote SFTP/FTPS download using a hidden subprocess (`CREATE_NO_WINDOW` on Windows).
pub async fn execute_remote_transfer(
    spec: &RemoteTransferSpec,
    local_dest: &Path,
) -> Result<(), String> {
    let plan = build_transfer_plan(spec, local_dest)?;

    let mut cmd = crate::packs::process::configure_hidden_command(&plan.program, &plan.args);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let output = cmd.output().await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            format!(
                "Required transfer binary '{}' not found on PATH",
                plan.program
            )
        } else {
            format!("Failed to launch '{}': {}", plan.program, e)
        }
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "{} exited with {:?}: {}",
            plan.program,
            output.status.code(),
            stderr.trim()
        ));
    }

    Ok(())
}

fn reject_control_chars(value: &str, field: &str) -> Result<(), String> {
    if value.chars().any(|c| c.is_control()) {
        return Err(format!("Invalid control character in {}", field));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn parses_sftp_url_with_defaults_and_credentials() {
        let spec = parse_sftp_or_ftps_url("sftp://deploy@files.example.com/releases/app.zip")
            .expect("should parse sftp url");
        assert_eq!(spec.protocol, RemoteProtocol::Sftp);
        assert_eq!(spec.host, "files.example.com");
        assert_eq!(spec.port, 22);
        assert_eq!(spec.username.as_deref(), Some("deploy"));
        assert_eq!(spec.password, None);
        assert_eq!(spec.remote_path, "/releases/app.zip");
        assert!(spec.strict_host_key);
        assert!(spec.verify_tls);
    }

    #[test]
    fn parses_ftps_url_with_custom_port() {
        let spec = parse_sftp_or_ftps_url("ftps://user:secret@ftp.example.com:2121/pub/data.bin")
            .expect("should parse ftps url");
        assert_eq!(spec.protocol, RemoteProtocol::Ftps);
        assert_eq!(spec.host, "ftp.example.com");
        assert_eq!(spec.port, 2121);
        assert_eq!(spec.username.as_deref(), Some("user"));
        assert_eq!(spec.password.as_deref(), Some("secret"));
        assert_eq!(spec.remote_path, "/pub/data.bin");
    }

    #[test]
    fn rejects_invalid_schemes_and_missing_paths() {
        assert!(parse_sftp_or_ftps_url("https://example.com/file.bin").is_err());
        assert!(parse_sftp_or_ftps_url("ftp://example.com/file.bin").is_err());
        assert!(parse_sftp_or_ftps_url("sftp://example.com").is_err());
        assert!(parse_sftp_or_ftps_url("sftp://example.com/").is_err());
        assert!(parse_sftp_or_ftps_url("not a url").is_err());
    }

    #[test]
    fn builds_injection_safe_plans_for_ftps_and_sftp() {
        let mut ftps = parse_sftp_or_ftps_url("ftps://u:p@example.com/dir/a.tar.gz").unwrap();
        ftps.verify_tls = false;
        let plan = build_transfer_plan(&ftps, &PathBuf::from("C:/tmp/a.tar.gz")).unwrap();
        assert!(plan.program.starts_with("curl"));
        assert!(plan.args.contains(&"--ssl-reqd".to_string()));
        assert!(plan.args.contains(&"--insecure".to_string()));
        assert!(!plan.args.iter().any(|a| a.contains("cmd")));

        let sftp = parse_sftp_or_ftps_url("sftp://alice@host.local:2222/var/log.txt").unwrap();
        let sftp_plan = build_transfer_plan(&sftp, &PathBuf::from("log.txt")).unwrap();
        assert!(sftp_plan.program.starts_with("sftp"));
        assert!(sftp_plan.args.contains(&"2222".to_string()));
        assert!(sftp_plan
            .args
            .contains(&"StrictHostKeyChecking=yes".to_string()));
        assert!(sftp_plan
            .args
            .contains(&"alice@host.local:/var/log.txt".to_string()));
    }

    #[test]
    fn rejects_control_chars_in_spec() {
        let mut spec = parse_sftp_or_ftps_url("sftp://host.local/file.txt").unwrap();
        spec.host = "host.local\n--evil".to_string();
        assert!(build_transfer_plan(&spec, Path::new("out.txt")).is_err());
    }
}
