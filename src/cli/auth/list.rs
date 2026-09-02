use anyhow::Result;

use crate::output;

/// Render a profile's `env` tag for the `auth list` table's `ENV` column
/// (BC-1.6.046 EC-1.6.046-1): `None` -> `-` placeholder; `Some(s)` -> the
/// shared `output::sanitize_env_display` transform (`Some("")` renders as a
/// blank cell — zero visible characters — never the `-` placeholder; the
/// two are NOT conflated). Shares the sanitizer with `auth status`'s text
/// `env` line (`status.rs`) — see BC-1.6.046 Ownership clause / BC-1.6.047
/// EC-1.6.047-3: "one shared sanitizer, two call sites."
pub(crate) fn render_env_column(env: Option<&str>) -> String {
    match env {
        None => "-".to_string(),
        Some(s) => crate::output::sanitize_env_display(s),
    }
}

/// Render the table-form output of `jr auth list`. The active profile is
/// marked with a leading `*`; others get a leading space so column widths
/// stay stable across rows. Status today is a coarse "do we have a URL on
/// file?" check — credential-store probing comes in Task 13.
pub(crate) fn render_list_table(global: &crate::config::GlobalConfig, active: &str) -> String {
    let mut rows: Vec<Vec<String>> = Vec::new();
    for (name, p) in &global.profiles {
        let marker = if name == active { "*" } else { " " };
        let auth = p.auth_method.as_deref().unwrap_or("?");
        let url = p.url.as_deref().unwrap_or("(unset)");
        // STATUS reflects CONFIG presence (URL on file), not credential
        // presence. `unset` is more accurate than the old `no-creds` label,
        // which suggested the keychain was missing entries when in reality
        // the profile entry simply lacks a URL.
        let status = if p.url.is_some() {
            "configured"
        } else {
            "unset"
        };
        rows.push(vec![
            format!("{marker} {name}"),
            url.to_string(),
            render_env_column(p.env.as_deref()),
            auth.to_string(),
            status.to_string(),
        ]);
    }
    crate::output::render_table(&["NAME", "URL", "ENV", "AUTH", "STATUS"], &rows)
}

/// Render the `--output json` form of `jr auth list`: an array of profile
/// objects keyed by name, with `active: true` on exactly one entry.
pub(crate) fn render_list_json(
    global: &crate::config::GlobalConfig,
    active: &str,
) -> Result<String> {
    let arr: Vec<serde_json::Value> = global
        .profiles
        .iter()
        .map(|(name, p)| {
            serde_json::json!({
                "name": name,
                "url": &p.url,
                "env": &p.env,
                "auth_method": &p.auth_method,
                "status": if p.url.is_some() { "configured" } else { "unset" },
                "active": name == active,
            })
        })
        .collect();
    output::render_json(&arr)
}

/// `jr auth list` — print every configured profile, marking the active one.
pub async fn handle_list(
    output: &crate::cli::OutputFormat,
    cli_profile: Option<&str>,
) -> Result<()> {
    let config = crate::config::Config::load_with(cli_profile)?;
    let rendered = match output {
        crate::cli::OutputFormat::Table => {
            render_list_table(&config.global, &config.active_profile_name)
        }
        crate::cli::OutputFormat::Json => {
            render_list_json(&config.global, &config.active_profile_name)?
        }
    };
    println!("{rendered}");
    Ok(())
}
