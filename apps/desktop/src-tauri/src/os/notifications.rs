use std::process::Stdio;

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub fn send_system_notification_internal(
    title: &str,
    message: &str,
    path: Option<&str>,
) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::io::Write;
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let escaped_title = xml_escape(title);
        let escaped_message = xml_escape(message);

        let (launch_attr, action_xml) = match path {
            Some(p) if !p.trim().is_empty() => {
                let uri = if p.starts_with("file://") {
                    p.to_string()
                } else {
                    let normalized = p.replace('\\', "/");
                    format!("file:///{}", normalized.trim_start_matches('/'))
                };
                let escaped_uri = xml_escape(&uri);
                (
                    format!(r#" activationType="protocol" launch="{}""#, escaped_uri),
                    format!(
                        r#"<actions><action content="Open Folder" arguments="{}" activationType="protocol" /></actions>"#,
                        escaped_uri
                    ),
                )
            }
            _ => (String::new(), String::new()),
        };

        let xml_payload = format!(
            r#"<toast{launch_attr}><visual><binding template="ToastGeneric"><text>{escaped_title}</text><text>{escaped_message}</text></binding></visual>{action_xml}<audio src="ms-winsoundevent:Notification.Default" /></toast>"#
        );

        let ps_script = format!(
            r#"[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null
[Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime] | Out-Null
$doc = New-Object Windows.Data.Xml.Dom.XmlDocument
$doc.LoadXml(@'
{}
'@)
$toast = New-Object Windows.UI.Notifications.ToastNotification $doc
try {{
    [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('com.edm.desktop').Show($toast)
}} catch {{
    [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('{{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}}\WindowsPowerShell\v1.0\powershell.exe').Show($toast)
}}
"#,
            xml_payload
        );

        let mut child = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("Failed to spawn PowerShell: {}", e))?;

        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(ps_script.as_bytes());
        }

        let output = child
            .wait_with_output()
            .map_err(|e| format!("Failed to wait on PowerShell: {}", e))?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            tracing::warn!("PowerShell toast notification warning: {}", err);
        }

        Ok(())
    }

    #[cfg(not(windows))]
    {
        tracing::info!(
            "System Notification: {} - {} (path: {:?})",
            title,
            message,
            path
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xml_escape() {
        assert_eq!(
            xml_escape("Rock & Roll <5> \"quote\" 'single'"),
            "Rock &amp; Roll &lt;5&gt; &quot;quote&quot; &apos;single&apos;"
        );
    }

    #[test]
    fn test_send_system_notification_smoke() {
        let res = send_system_notification_internal("Test Notification", "Test Message", None);
        assert!(res.is_ok());
    }
}
