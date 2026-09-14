//! Composing a connection string from separate fields.
//!
//! Typing a DSN by hand is where connections fail: an unencoded `@` in a password, a missing
//! database name, a TLS flag spelled the way another tool spells it. The dialog collects the parts
//! and builds the string here, so the rules live in one tested place instead of in the user's head.

/// What the user fills in instead of writing a connection string.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Fields {
    pub host: String,
    pub port: String,
    pub database: String,
    pub user: String,
    pub password: String,
    /// Engine-specific TLS token from `tls_choices`.
    pub tls: String,
}

/// Shown in place of the password wherever the composed string is displayed.
pub const MASK: &str = "••••••••";

pub fn default_port(kind: &str) -> &'static str {
    match kind {
        "postgres" => "5432",
        "mysql" | "mariadb" => "3306",
        "mssql" => "1433",
        _ => "",
    }
}

/// TLS settings offered per engine, as (value written into the string, label).
pub fn tls_choices(kind: &str) -> &'static [(&'static str, &'static str)] {
    match kind {
        "postgres" => &[
            ("require", "Required"),
            ("verify-full", "Required, verify server identity"),
            ("verify-ca", "Required, verify certificate"),
            ("prefer", "Preferred"),
            ("disable", "Disabled"),
        ],
        "mysql" | "mariadb" => &[
            ("REQUIRED", "Required"),
            ("VERIFY_IDENTITY", "Required, verify server identity"),
            ("VERIFY_CA", "Required, verify certificate"),
            ("PREFERRED", "Preferred"),
            ("DISABLED", "Disabled"),
        ],
        "mssql" => &[
            ("encrypt", "Required"),
            ("encrypt-trust", "Required, trust server certificate"),
            ("plain", "Disabled"),
        ],
        _ => &[],
    }
}

pub fn default_tls(kind: &str) -> &'static str {
    tls_choices(kind)
        .first()
        .map(|(value, _)| *value)
        .unwrap_or("")
}

/// Percent-encode a URL component, keeping only the unreserved set so credentials survive intact.
fn encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// Quote an ADO.NET value. The parser tiberius uses reads a braced value up to the first `}`, with
/// no way to escape one, so a value containing `}` cannot be represented at all.
fn ado_value(label: &str, value: &str) -> Result<String, String> {
    if !value.is_ascii() {
        return Err(format!(
            "SQL Server connection strings accept ASCII only. Remove the accented characters from the {label}."
        ));
    }
    if value.contains('}') {
        return Err(format!(
            "A SQL Server {label} cannot contain }}. Change it on the server, or use the connection string field."
        ));
    }
    if value.contains([';', '=', '{']) || value.trim() != value {
        return Ok(format!("{{{value}}}"));
    }
    Ok(value.to_string())
}

fn host_authority(host: &str, port: &str) -> Result<String, String> {
    let host = host.trim();
    if host.is_empty() {
        return Err("Enter the server host.".into());
    }
    if host.contains(['/', '@', ' ', '?', '#']) {
        return Err("The host must not contain a slash, @, ?, # or a space.".into());
    }
    // A bare IPv6 address carries colons, which a URL authority only accepts inside brackets.
    let host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_string()
    };
    match port_number(port)? {
        Some(port) => Ok(format!("{host}:{port}")),
        None => Ok(host),
    }
}

fn port_number(port: &str) -> Result<Option<u16>, String> {
    let port = port.trim();
    if port.is_empty() {
        return Ok(None);
    }
    port.parse::<u16>()
        .ok()
        .filter(|port| *port > 0)
        .map(Some)
        .ok_or_else(|| "The port must be a number between 1 and 65535.".into())
}

fn required<'a>(value: &'a str, message: &str) -> Result<&'a str, String> {
    let value = value.trim();
    if value.is_empty() {
        Err(message.to_string())
    } else {
        Ok(value)
    }
}

fn build(kind: &str, fields: &Fields, mask: bool) -> Result<String, String> {
    let user = required(&fields.user, "Enter the database user.")?;
    let database = required(&fields.database, "Enter the database name.")?;
    match kind {
        "postgres" | "mysql" | "mariadb" => {
            let authority = host_authority(&fields.host, &fields.port)?;
            let secret = if mask {
                MASK.to_string()
            } else {
                encode(&fields.password)
            };
            let credentials = if fields.password.is_empty() {
                encode(user)
            } else {
                format!("{}:{secret}", encode(user))
            };
            let (scheme, parameter) = if kind == "postgres" {
                ("postgresql", "sslmode")
            } else {
                ("mysql", "ssl-mode")
            };
            let tls = if fields.tls.is_empty() {
                default_tls(kind)
            } else {
                &fields.tls
            };
            Ok(format!(
                "{scheme}://{credentials}@{authority}/{}?{parameter}={tls}",
                encode(database)
            ))
        }
        "mssql" => {
            let host = required(&fields.host, "Enter the server host.")?;
            let server = match port_number(&fields.port)? {
                Some(port) => format!("{host},{port}"),
                None => host.to_string(),
            };
            // Validate before masking, so the preview reports what the format cannot carry
            // instead of looking fine until the connection is attempted.
            let quoted = ado_value("password", &fields.password)?;
            let secret = if mask { MASK.to_string() } else { quoted };
            let encryption = match fields.tls.as_str() {
                "plain" => "Encrypt=false",
                "encrypt-trust" => "Encrypt=true;TrustServerCertificate=true",
                _ => "Encrypt=true",
            };
            Ok(format!(
                "Server=tcp:{};Database={};User ID={};Password={secret};{encryption}",
                ado_value("host", &server)?,
                ado_value("database name", database)?,
                ado_value("user", user)?,
            ))
        }
        _ => Err("Choose a database engine that accepts a host and user.".into()),
    }
}

/// The connection string these fields describe.
pub fn compose(kind: &str, fields: &Fields) -> Result<String, String> {
    build(kind, fields, false)
}

/// The same string with the password replaced, safe to show on screen.
pub fn preview(kind: &str, fields: &Fields) -> Result<String, String> {
    build(kind, fields, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields() -> Fields {
        Fields {
            host: "db.internal".into(),
            port: String::new(),
            database: "shop".into(),
            user: "alice".into(),
            password: "p@ss word#1".into(),
            tls: String::new(),
        }
    }

    #[test]
    fn percent_encodes_credentials_that_would_break_a_url() {
        assert_eq!(
            compose("postgres", &fields()).unwrap(),
            "postgresql://alice:p%40ss%20word%231@db.internal/shop?sslmode=require"
        );
        let mut mysql = fields();
        mysql.port = "3307".into();
        mysql.tls = "VERIFY_IDENTITY".into();
        assert_eq!(
            compose("mysql", &mysql).unwrap(),
            "mysql://alice:p%40ss%20word%231@db.internal:3307/shop?ssl-mode=VERIFY_IDENTITY"
        );
        // MariaDB speaks the same URL shape as MySQL.
        assert!(compose("mariadb", &mysql).unwrap().starts_with("mysql://"));
    }

    #[test]
    fn encodes_multibyte_secrets_byte_by_byte() {
        let mut with_accent = fields();
        with_accent.password = "clé".into();
        assert_eq!(
            compose("postgres", &with_accent).unwrap(),
            "postgresql://alice:cl%C3%A9@db.internal/shop?sslmode=require"
        );
    }

    #[test]
    fn omits_the_password_separator_when_there_is_none() {
        let mut anonymous = fields();
        anonymous.password = String::new();
        assert_eq!(
            compose("postgres", &anonymous).unwrap(),
            "postgresql://alice@db.internal/shop?sslmode=require"
        );
    }

    #[test]
    fn brackets_a_bare_ipv6_host() {
        let mut ipv6 = fields();
        ipv6.host = "2001:db8::1".into();
        ipv6.port = "5432".into();
        assert_eq!(
            compose("postgres", &ipv6).unwrap(),
            "postgresql://alice:p%40ss%20word%231@[2001:db8::1]:5432/shop?sslmode=require"
        );
    }

    #[test]
    fn quotes_ado_values_and_refuses_what_the_format_cannot_carry() {
        let mut server = fields();
        server.password = "pa;ss".into();
        server.tls = "encrypt-trust".into();
        assert_eq!(
            compose("mssql", &server).unwrap(),
            "Server=tcp:db.internal;Database=shop;User ID=alice;Password={pa;ss};Encrypt=true;TrustServerCertificate=true"
        );
        let mut brace = fields();
        brace.password = "pa}ss".into();
        assert!(compose("mssql", &brace)
            .unwrap_err()
            .contains("cannot contain"));
        let mut accent = fields();
        accent.password = "clé".into();
        assert!(compose("mssql", &accent).unwrap_err().contains("ASCII"));
    }

    #[test]
    fn reports_the_missing_field_rather_than_a_broken_string() {
        for (change, expected) in [
            (
                (|f: &mut Fields| f.host = String::new()) as fn(&mut Fields),
                "host",
            ),
            (|f: &mut Fields| f.user = String::new(), "user"),
            (|f: &mut Fields| f.database = String::new(), "database name"),
            (|f: &mut Fields| f.port = "70000".into(), "65535"),
            (|f: &mut Fields| f.host = "db/other".into(), "slash"),
        ] {
            let mut broken = fields();
            change(&mut broken);
            let error = compose("postgres", &broken).unwrap_err();
            assert!(error.contains(expected), "{error}");
        }
    }

    #[test]
    fn the_preview_reports_what_the_format_cannot_carry() {
        let mut brace = fields();
        brace.password = "pa}ss".into();
        assert!(preview("mssql", &brace).is_err());
        brace.host = String::new();
        assert!(preview("postgres", &brace).unwrap_err().contains("host"));
    }

    #[test]
    fn the_preview_never_carries_the_password() {
        let shown = preview("postgres", &fields()).unwrap();
        assert!(shown.contains(MASK));
        assert!(!shown.contains("p%40ss"));
        let server = preview("mssql", &fields()).unwrap();
        assert!(server.contains(MASK));
        // "Password=" legitimately contains "word", so look for the secret itself.
        assert!(!server.contains("p@ss"));
    }

    #[test]
    fn every_engine_offers_a_default_port_and_tls_setting() {
        for kind in ["postgres", "mysql", "mariadb", "mssql"] {
            assert!(!default_port(kind).is_empty(), "{kind}");
            assert!(!default_tls(kind).is_empty(), "{kind}");
            assert!(tls_choices(kind)
                .iter()
                .any(|(v, _)| *v == default_tls(kind)));
        }
        assert!(tls_choices("sqlite").is_empty());
    }
}
