use anyhow::Result;
use colored::Colorize;

use crate::api::client::JiraClient;
use crate::cli::{OutputFormat, UserCommand, resolve_effective_limit};
use crate::config::Config;
use crate::error::JrError;
use crate::output;
use crate::types::jira::User;

/// Resolves `jr user list`'s effective project key from the post-clap
/// `--project` value (already local-vs-global resolved by clap's own
/// global-value propagation) plus the configured default fallback chain.
///
/// `pub(crate)` pure resolver mirroring
/// `src/cli/field.rs::resolve_m2_project`'s signature style, per BC-X.7.002
/// Fix step 4: delegates entirely to `Config::project_key`'s existing
/// fallback chain (`.jr.toml` project, then the active profile's configured
/// `project` default) — no new `Config`/`ProfileConfig` accessor is added.
pub(crate) fn resolve_user_list_project(
    cli_project: Option<&str>,
    config: &Config,
) -> Option<String> {
    config.project_key(cli_project)
}

pub async fn handle(
    command: UserCommand,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
) -> Result<()> {
    match command {
        UserCommand::Search { query, limit, all } => {
            handle_search(&query, limit, all, output_format, client).await
        }
        UserCommand::List {
            project,
            limit,
            all,
        } => {
            handle_list(
                project.as_deref(),
                limit,
                all,
                output_format,
                config,
                client,
            )
            .await
        }
        UserCommand::View { account_id } => handle_view(&account_id, output_format, client).await,
    }
}

async fn handle_search(
    query: &str,
    limit: Option<u32>,
    all: bool,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let effective = resolve_effective_limit(limit, all);
    let mut users = if all {
        client.search_users_all(query).await?
    } else {
        client.search_users(query).await?
    };
    if let Some(cap) = effective {
        users.truncate(cap as usize);
    }
    print_user_list(&users, output_format)
}

async fn handle_list(
    project: Option<&str>,
    limit: Option<u32>,
    all: bool,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
) -> Result<()> {
    // BC-X.7.002 Fix step 4: unconditionally resolve via local flag > global
    // flag (both already reflected in the post-clap `project` value) >
    // configured default > exit 64, before any HTTP call.
    let resolved_project = resolve_user_list_project(project, config).ok_or_else(|| {
        JrError::UserError(
            "No project configured. Run \"jr init\" or pass --project. \
             Run \"jr project list\" to see available projects."
                .into(),
        )
    })?;

    let effective = resolve_effective_limit(limit, all);
    let mut users = if all {
        client
            .search_assignable_users_by_project_all("", &resolved_project)
            .await?
    } else {
        client
            .search_assignable_users_by_project("", &resolved_project)
            .await?
    };
    if let Some(cap) = effective {
        users.truncate(cap as usize);
    }
    print_user_list(&users, output_format)
}

async fn handle_view(
    account_id: &str,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let user = match client.get_user(account_id).await {
        Ok(u) => u,
        Err(e) => {
            if let Some(JrError::ApiError { status, .. }) = e.downcast_ref::<JrError>()
                && (*status == 404 || *status == 400)
            {
                return Err(JrError::UserError(format!(
                    "User with accountId '{account_id}' not found."
                ))
                .into());
            }
            return Err(e);
        }
    };

    let rows = vec![
        vec!["Account ID".into(), user.account_id.clone()],
        vec!["Display Name".into(), user.display_name.clone()],
        vec![
            "Email".into(),
            user.email_address.clone().unwrap_or_else(|| "—".into()),
        ],
        vec!["Active".into(), format_active(user.active)],
    ];

    output::print_output(output_format, &["Field", "Value"], &rows, &user)
}

fn print_user_list(users: &[User], output_format: &OutputFormat) -> Result<()> {
    let rows: Vec<Vec<String>> = users.iter().map(format_user_row).collect();
    output::print_output(
        output_format,
        &["Display Name", "Email", "Active", "Account ID"],
        &rows,
        &users,
    )
}

fn format_user_row(user: &User) -> Vec<String> {
    vec![
        user.display_name.clone(),
        user.email_address.clone().unwrap_or_else(|| "—".into()),
        format_active(user.active),
        user.account_id.clone(),
    ]
}

fn format_active(active: Option<bool>) -> String {
    match active {
        Some(true) => "✓".green().to_string(),
        Some(false) => "✗".red().to_string(),
        None => "—".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{GlobalConfig, ProfileConfig, ProjectConfig};
    use proptest::prelude::*;

    /// Builds a `Config` with the active profile named `"default"`, an
    /// optional `.jr.toml`-equivalent `project` value, and an optional
    /// profile-level `project` default — the two independently-controllable
    /// sources `Config::project_key` falls back through.
    fn make_config(jr_toml_project: Option<String>, profile_project: Option<String>) -> Config {
        let mut profiles = std::collections::BTreeMap::new();
        profiles.insert(
            "default".to_string(),
            ProfileConfig {
                project: profile_project,
                ..ProfileConfig::default()
            },
        );
        Config {
            global: GlobalConfig {
                default_profile: Some("default".to_string()),
                profiles,
                ..GlobalConfig::default()
            },
            project: ProjectConfig {
                project: jr_toml_project,
                ..ProjectConfig::default()
            },
            active_profile_name: "default".into(),
        }
    }

    proptest! {
        /// AC-003 / AC-006 (BC-X.7.002 Fix step 4, VP-USER-LIST-PROJECT-001(b)):
        /// `resolve_user_list_project` over the full presence space of
        /// `cli_project` (`Some(C)`, `Some("")` — EC-X.7.002-6, `None`)
        /// crossed with the four configured-source cells (neither,
        /// `.jr.toml`-only, profile-only, both — `.jr.toml` wins over the
        /// profile default, EC-X.7.002-5's caveat).
        #[test]
        fn test_bc_x_7_002_resolve_user_list_project_presence_space(
            c in "c-[a-zA-Z0-9]{1,8}",
            j in "j-[a-zA-Z0-9]{1,8}",
            p in "p-[a-zA-Z0-9]{1,8}",
        ) {
            let cells: [(Option<String>, Option<String>, Option<String>); 4] = [
                (None, None, None),
                (Some(j.clone()), None, Some(j.clone())),
                (None, Some(p.clone()), Some(p.clone())),
                (Some(j.clone()), Some(p.clone()), Some(j.clone())),
            ];
            for (jr_toml, profile, expected_when_cli_absent) in cells {
                let config = make_config(jr_toml, profile);

                // cli_project = Some(C) -> Some(C) in every configured cell.
                prop_assert_eq!(
                    resolve_user_list_project(Some(c.as_str()), &config),
                    Some(c.clone())
                );

                // EC-X.7.002-6: cli_project = Some("") -> Some(""), configured default not consulted.
                prop_assert_eq!(
                    resolve_user_list_project(Some(""), &config),
                    Some(String::new())
                );

                // cli_project = None -> configured fallback chain (.jr.toml > profile > None).
                prop_assert_eq!(
                    resolve_user_list_project(None, &config),
                    expected_when_cli_absent
                );
            }
        }
    }

    #[test]
    fn row_shows_display_name_email_and_id() {
        let user = User {
            account_id: "acc-1".into(),
            display_name: "Alice".into(),
            email_address: Some("alice@acme.io".into()),
            active: Some(true),
        };
        let row = format_user_row(&user);
        assert_eq!(row[0], "Alice");
        assert_eq!(row[1], "alice@acme.io");
        assert!(row[2].contains('✓'));
        assert_eq!(row[3], "acc-1");
    }

    #[test]
    fn row_renders_dash_for_missing_email() {
        let user = User {
            account_id: "acc-2".into(),
            display_name: "Privacy User".into(),
            email_address: None,
            active: Some(true),
        };
        let row = format_user_row(&user);
        assert_eq!(row[1], "—");
    }

    #[test]
    fn active_formatter_handles_missing() {
        assert_eq!(format_active(None), "—");
    }

    // ── format_active structural styling (BC-7.1.006, FIX-P5-001) ──────
    //
    // `colored`'s global override (`colored::control::set_override`) is a
    // single process-wide `AtomicBool` (see `colored::control::SHOULD_COLORIZE`,
    // colored 3.1.1) shared by every thread in this test binary. Rust test
    // threads run in parallel by default, so any test that forces the
    // override must serialize against every OTHER test in this binary that
    // could observe color state — hence `COLOR_OVERRIDE_LOCK` below, held
    // for the guard's whole lifetime, plus a `Drop` impl that always
    // restores via `unset_override()` (including on panic/unwind — this
    // crate's test profile is NOT `panic = "abort"`; only
    // `[profile.release]` is), so a failing assertion inside the guarded
    // block can't leave color permanently forced on for later tests.
    static COLOR_OVERRIDE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// RAII guard forcing `colored`'s global override for its lifetime,
    /// serialized against `COLOR_OVERRIDE_LOCK` so concurrent test threads
    /// in this binary can't race on the process-wide `colored::control`
    /// static (see module doc above). Always restores via
    /// `colored::control::unset_override()` on drop.
    struct ForcedColorOverride {
        _guard: std::sync::MutexGuard<'static, ()>,
    }

    impl ForcedColorOverride {
        fn new(enabled: bool) -> Self {
            let guard = COLOR_OVERRIDE_LOCK
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            colored::control::set_override(enabled);
            Self { _guard: guard }
        }
    }

    impl Drop for ForcedColorOverride {
        fn drop(&mut self) {
            colored::control::unset_override();
        }
    }

    /// BC-7.1.006: once `render_table` sanitizes every cell (via the new
    /// `output::sanitize_table_cell`), a cell whose ANSI styling is baked
    /// into the `String` itself — as `format_active` does TODAY via
    /// `colored`'s `.to_string()` — would have that styling stripped
    /// alongside any hostile payload, silently breaking jr's own
    /// intentional Active-column coloring. The BC requires `format_active`'s
    /// styling to move to structural `comfy_table::Cell` attributes,
    /// meaning `format_active` itself must return the BARE glyph — no ANSI
    /// bytes embedded in the `String` — regardless of whether color is
    /// currently enabled.
    ///
    /// Deterministic without a real TTY: `colored`'s own suppression when
    /// stdout isn't a terminal (the default under `cargo test`, where
    /// stdout is captured) would make this assertion trivially true today
    /// for the WRONG reason — not because `format_active` is structural,
    /// but because `colored` isn't emitting ANSI at all in this process.
    /// Forcing the override ON via `ForcedColorOverride` closes that gap:
    /// with color forced on, today's `format_active` DOES embed ANSI bytes
    /// (`"\x1b[32m✓\x1b[0m"` / `"\x1b[31m✗\x1b[0m"`), so this test fails
    /// against current production code and will only pass once
    /// `format_active` returns the bare glyph unconditionally.
    #[test]
    fn test_bc_7_1_006_format_active_returns_bare_glyph_no_esc_bytes_when_color_forced_on() {
        let _color = ForcedColorOverride::new(true);

        let active_glyph = format_active(Some(true));
        let inactive_glyph = format_active(Some(false));

        assert_eq!(
            active_glyph, "✓",
            "format_active must return the bare glyph — styling belongs on a \
             structural Cell attribute (BC-7.1.006), not ANSI bytes embedded \
             in the cell String: got {active_glyph:?}"
        );
        assert_eq!(
            inactive_glyph, "✗",
            "format_active must return the bare glyph — styling belongs on a \
             structural Cell attribute (BC-7.1.006), not ANSI bytes embedded \
             in the cell String: got {inactive_glyph:?}"
        );
        assert!(
            !active_glyph.contains('\u{1b}'),
            "raw ESC byte embedded in format_active's returned String: {active_glyph:?}"
        );
        assert!(
            !inactive_glyph.contains('\u{1b}'),
            "raw ESC byte embedded in format_active's returned String: {inactive_glyph:?}"
        );
    }

    /// GREEN today, justified: the Active column glyph itself (✓/✗) is
    /// already present in `print_user_list`'s rendered table output — this
    /// is pre-existing correctness, unrelated to BC-7.1.006's sanitization
    /// fix. Included as a defense-in-depth regression guard: once
    /// `format_active`'s styling moves to structural `Cell` attributes
    /// (the test above), the glyph itself must still reach the rendered
    /// table — only its coloring mechanism changes, not its visibility.
    #[test]
    fn test_bc_7_1_006_rendered_table_still_shows_active_glyph_for_active_and_inactive_users() {
        let active_user = User {
            account_id: "acc-1".into(),
            display_name: "Active User".into(),
            email_address: Some("active@acme.io".into()),
            active: Some(true),
        };
        let inactive_user = User {
            account_id: "acc-2".into(),
            display_name: "Inactive User".into(),
            email_address: Some("inactive@acme.io".into()),
            active: Some(false),
        };
        let rows: Vec<Vec<String>> = vec![&active_user, &inactive_user]
            .into_iter()
            .map(format_user_row)
            .collect();

        let output =
            output::render_table(&["Display Name", "Email", "Active", "Account ID"], &rows);

        assert!(
            output.contains('✓'),
            "active user's glyph must still be visible in the rendered table: {output:?}"
        );
        assert!(
            output.contains('✗'),
            "inactive user's glyph must still be visible in the rendered table: {output:?}"
        );
    }

    /// GREEN today, justified: this test validates the GENERAL TECHNIQUE
    /// BC-7.1.006 prescribes for `format_active`'s refactor — structural
    /// `comfy_table::Cell` styling (`Cell::new(glyph).fg(Color::Green)`)
    /// survives even though the cell TEXT itself (the bare glyph, once
    /// `format_active` is fixed) will pass through `sanitize_table_cell`
    /// unchanged. It exercises `comfy_table` directly (not `format_active`
    /// or jr's `render_table`, whose signature is `&[Vec<String>]` and
    /// cannot carry a styled `Cell` without an implementation change that
    /// is out of scope for this Red Gate pass — F6's job), so it does not
    /// pin jr's own code and is expected to be GREEN both before and after
    /// F6 lands.
    ///
    /// `comfy_table::Table::should_style()` is gated on `is_tty()`
    /// (`std::io::stdout().is_terminal()`), which is false under `cargo
    /// test` (stdout is captured) — `colored`'s own override mechanism
    /// does NOT affect `comfy_table`'s independent TTY gate. Determinism
    /// here comes from `Table::force_no_tty().enforce_styling()`, which
    /// `comfy_table` documents as the supported way to force styled output
    /// regardless of the ambient TTY/environment.
    #[test]
    fn test_bc_7_1_006_structural_cell_styling_technique_survives_rendering() {
        use comfy_table::{Cell, Color, Table};

        let mut table = Table::new();
        table.force_no_tty().enforce_styling();
        table.set_header(vec!["Active"]);
        table.add_row(vec![Cell::new("✓").fg(Color::Green)]);
        table.add_row(vec![Cell::new("✗").fg(Color::Red)]);

        let rendered = table.to_string();

        assert!(
            rendered.contains('\u{1b}'),
            "expected ANSI styling bytes when Cell::fg + enforce_styling is \
             used, got: {rendered:?}"
        );
        assert!(rendered.contains('✓'));
        assert!(rendered.contains('✗'));
    }
}
