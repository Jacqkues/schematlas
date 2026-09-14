use crate::error::{AppError, Result};
use sqlx::{
    mysql::{MySqlConnectOptions, MySqlSslMode},
    Connection, MySqlConnection,
};

fn invalid(message: &str) -> AppError {
    AppError::Validation(message.into())
}

/// Parse connection options without logging the URL or returning driver errors that may contain it.
fn options(dsn: &str) -> Result<MySqlConnectOptions> {
    let mut url = url::Url::parse(dsn)
        .map_err(|_| invalid("Invalid MySQL URL. Use mysql://user:password@host:3306/database."))?;
    if url.scheme() != "mysql" || url.host_str().is_none() || url.fragment().is_some() {
        return Err(invalid("Use a mysql:// URL with a host. Percent-encode special characters in credentials, including # as %23."));
    }
    if url.username().is_empty() {
        return Err(invalid("Include your MySQL username in the URL: mysql://user:password@host:3306/database. Enter credentials only in the app."));
    }
    if url.path().trim_start_matches('/').is_empty() {
        return Err(invalid(
            "Include a database name in the MySQL connection URL.",
        ));
    }
    let mut explicit_mode = None;
    let mut use_ssl = None;
    let mut require_ssl = None;
    let mut verify_certificate = None;
    let mut remaining = Vec::new();
    for (key, value) in url.query_pairs() {
        match key.as_ref() {
            "ssl-mode" | "sslmode" | "sslMode" => {
                if explicit_mode.is_some() {
                    return Err(invalid("Specify the MySQL SSL mode only once."));
                }
                explicit_mode = Some(value.parse::<MySqlSslMode>().map_err(|_| invalid("Invalid MySQL SSL mode. Use DISABLED, PREFERRED, REQUIRED, VERIFY_CA, or VERIFY_IDENTITY."))?);
            }
            "useSSL" | "requireSSL" | "verifyServerCertificate" => {
                let slot = match key.as_ref() {
                    "useSSL" => &mut use_ssl,
                    "requireSSL" => &mut require_ssl,
                    _ => &mut verify_certificate,
                };
                if slot.is_some() {
                    return Err(invalid("Specify each MySQL TLS option only once."));
                }
                *slot = Some(match value.to_ascii_lowercase().as_str() {
                    "true" => true,
                    "false" => false,
                    _ => return Err(invalid("MySQL useSSL, requireSSL, and verifyServerCertificate must be true or false.")),
                });
            }
            _ => remaining.push((key.into_owned(), value.into_owned())),
        }
    }
    // Follow Connector/J's documented translation; an explicit SSL mode takes precedence.
    let mode = explicit_mode.unwrap_or_else(|| {
        if use_ssl == Some(false) {
            MySqlSslMode::Disabled
        } else if verify_certificate == Some(true) {
            MySqlSslMode::VerifyCa
        } else if require_ssl == Some(true) {
            MySqlSslMode::Required
        } else {
            MySqlSslMode::Preferred
        }
    });
    url.set_query(None);
    if !remaining.is_empty() {
        url.query_pairs_mut().extend_pairs(remaining);
    }
    let options = url.as_str().parse::<MySqlConnectOptions>().map_err(|_| {
        invalid("Invalid MySQL connection options. Check URL encoding and parameter values.")
    })?;
    Ok(options.ssl_mode(mode))
}

pub(crate) async fn connect(dsn: &str) -> Result<MySqlConnection> {
    MySqlConnection::connect_with(&options(dsn)?)
        .await
        .map_err(connection_error)
}

fn connection_error(error: sqlx::Error) -> AppError {
    let message = match &error {
        sqlx::Error::Database(database) => match database.try_downcast_ref::<sqlx::mysql::MySqlDatabaseError>().map(|e| e.number()) {
            Some(1045 | 1698) => "MySQL authentication failed. Check the username and password in the URL and whether this account may connect from your computer.",
            Some(1044) => "The MySQL account does not have access to the selected database.",
            Some(1049) => "MySQL could not find the selected database. Check its name in the URL.",
            Some(1130) => "The MySQL server does not allow connections from this computer. Check the account's allowed hosts or IP allowlist.",
            Some(3159) => "The MySQL server requires an encrypted connection. Add ssl-mode=REQUIRED to the URL.",
            _ => return AppError::Database(error),
        },
        sqlx::Error::Tls(_) => "MySQL TLS negotiation failed. Check the server's TLS support and the selected SSL mode or CA certificate.",
        sqlx::Error::Io(_) => "Could not reach the MySQL server, or the connection was interrupted. Check the hostname, port, VPN, firewall, and IP allowlist.",
        sqlx::Error::Protocol(_) => "The MySQL handshake failed. Check that the port serves MySQL and that the server's authentication method is supported.",
        _ => return AppError::Database(error),
    };
    invalid(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translates_jdbc_tls_flags_and_preserves_url_fields() {
        let parsed = options("mysql://alice:p%40ss@localhost:24004/demo_app?useSSL=true&requireSSL=true&verifyServerCertificate=false&charset=utf8mb4").unwrap();
        assert!(matches!(parsed.get_ssl_mode(), MySqlSslMode::Required));
        assert_eq!(parsed.get_username(), "alice");
        assert_eq!(parsed.get_host(), "localhost");
        assert_eq!(parsed.get_port(), 24004);
        assert_eq!(parsed.get_database(), Some("demo_app"));
    }

    #[test]
    fn honors_verification_and_explicit_mode_precedence() {
        for (query, expected) in [
            ("useSSL=true&verifyServerCertificate=true", "VerifyCa"),
            ("useSSL=false", "Disabled"),
            (
                "useSSL=true&requireSSL=false&verifyServerCertificate=false",
                "Preferred",
            ),
            ("sslMode=VERIFY_IDENTITY&useSSL=false", "VerifyIdentity"),
            ("ssl-mode=REQUIRED", "Required"),
            ("sslmode=VERIFY_CA", "VerifyCa"),
            ("ssl-mode=verify_identity", "VerifyIdentity"),
        ] {
            let parsed = options(&format!("mysql://alice:secret@localhost/demo?{query}")).unwrap();
            assert_eq!(format!("{:?}", parsed.get_ssl_mode()), expected);
        }
    }

    #[test]
    fn rejects_incomplete_or_ambiguous_options_without_exposing_credentials() {
        for dsn in [
            "mysql://localhost/demo",
            "mysql://alice:secret@localhost/",
            "postgres://alice:secret@localhost/demo",
            "mysql://alice:secret@localhost/demo?requireSSL=secret",
            "mysql://alice:secret@localhost/demo?sslMode=secret",
            "mysql://alice:secret@localhost/demo?ssl-mode=REQUIRED&sslMode=DISABLED",
            "mysql://alice:secret@localhost/demo?useSSL=true&useSSL=false",
            "mysql://alice:secret@localhost/demo#secret",
        ] {
            let error = options(dsn).unwrap_err().to_string();
            assert!(!error.contains("secret"));
        }
    }

    #[tokio::test]
    async fn connect_rejects_a_bad_url_before_opening_a_socket() {
        let error = connect("postgres://alice:secret@localhost/demo")
            .await
            .unwrap_err();
        assert!(matches!(error, AppError::Validation(_)));
        assert!(!error.to_string().contains("secret"));
    }

    #[test]
    fn connection_errors_do_not_expose_driver_details() {
        let error = connection_error(sqlx::Error::Tls("private connection details".into()));
        assert!(error.to_string().contains("TLS"));
        assert!(!error.to_string().contains("private connection details"));
    }
}
