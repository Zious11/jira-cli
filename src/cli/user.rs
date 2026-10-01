use anyhow::Result;
use comfy_table::Color;

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

    // BC-7.1.006: the Active row's color is a structural Cell attribute
    // (via active_cell), never ANSI bytes embedded in the cell String.
    let rows = vec![
        vec![
            output::StyledCell::plain("Account ID"),
            output::StyledCell::plain(user.account_id.clone()),
        ],
        vec![
            output::StyledCell::plain("Display Name"),
            output::StyledCell::plain(user.display_name.clone()),
        ],
        vec![
            output::StyledCell::plain("Email"),
            output::StyledCell::plain(user.email_address.clone().unwrap_or_else(|| "—".into())),
        ],
        vec![
            output::StyledCell::plain("Active"),
            active_cell(user.active),
        ],
    ];

    output::print_output_with_styles(output_format, &["Field", "Value"], &rows, &user)
}

fn print_user_list(users: &[User], output_format: &OutputFormat) -> Result<()> {
    let rows: Vec<Vec<output::StyledCell>> = users.iter().map(format_user_row_styled).collect();
    output::print_output_with_styles(
        output_format,
        &["Display Name", "Email", "Active", "Account ID"],
        &rows,
        &users,
    )
}

/// Styled-cell sibling of [`format_user_row`] for `print_user_list`'s
/// table-mode rendering (BC-7.1.006): identical to `format_user_row`
/// except the Active column carries its green/red coloring as a
/// structural `Cell` attribute (via [`active_cell`]) rather than as plain
/// text. Built on top of `format_user_row` itself so that function stays a
/// live production call site, not test-only dead code.
fn format_user_row_styled(user: &User) -> Vec<output::StyledCell> {
    let plain = format_user_row(user);
    vec![
        output::StyledCell::plain(plain[0].clone()),
        output::StyledCell::plain(plain[1].clone()),
        active_cell(user.active),
        output::StyledCell::plain(plain[3].clone()),
    ]
}

fn format_user_row(user: &User) -> Vec<String> {
    vec![
        user.display_name.clone(),
        user.email_address.clone().unwrap_or_else(|| "—".into()),
        format_active(user.active),
        user.account_id.clone(),
    ]
}

/// The Active column's bare glyph (BC-7.1.006) — no ANSI bytes embedded.
/// Styling now lives on a structural `comfy_table::Cell` attribute, built
/// by [`active_cell`], since a cell string can no longer carry its own
/// ANSI escape bytes once `render_table`/`render_table_with_styles`
/// sanitize every cell (this BC's whole point — see
/// `output::sanitize_table_cell`).
fn format_active(active: Option<bool>) -> String {
    match active {
        Some(true) => "✓".into(),
        Some(false) => "✗".into(),
        None => "—".into(),
    }
}

/// Builds the Active column's structurally-styled cell (BC-7.1.006):
/// green/red applied via `comfy_table::Cell::fg`, never as ANSI bytes in
/// the cell text (which is always the bare [`format_active`] glyph).
///
/// Gated on `colored::control::SHOULD_COLORIZE.should_colorize()` — the
/// same process-wide flag `main.rs` forces to `false` for `--no-color` /
/// `NO_COLOR` (`colored::control::set_override(false)`). Since CR-2
/// (D-396/FIX-P5-002), `output::render_table_with_styles` enforces the same
/// gate structurally for every `StyledCell`, so this check is now
/// redundant but harmless; it is kept so `active_cell` itself never
/// produces a colored cell under `--no-color`/`NO_COLOR`, independent of
/// the renderer. (`comfy_table`'s own TTY-based `Table::should_style()`
/// knows nothing about jr's `--no-color` flag or `NO_COLOR`.)
fn active_cell(active: Option<bool>) -> output::StyledCell {
    let glyph = format_active(active);
    if !colored::control::SHOULD_COLORIZE.should_colorize() {
        return output::StyledCell::plain(glyph);
    }
    match active {
        Some(true) => output::StyledCell::colored(glyph, Color::Green),
        Some(false) => output::StyledCell::colored(glyph, Color::Red),
        None => output::StyledCell::plain(glyph),
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
    // Color-override tests use the single crate-wide guard in
    // `output::color_test_lock` (FIX-P5-003, P2-002/CR2-2): one mutex for
    // every test touching `colored::control`'s process-global override, so
    // tests in different modules cannot race each other.
    use crate::output::color_test_lock::ColorOverride as ForcedColorOverride;

    /// BC-7.1.006: `render_table`/`render_table_with_styles` sanitize every
    /// cell (via `output::sanitize_table_cell`), so a cell whose ANSI
    /// styling was baked into the `String` itself would have that styling
    /// stripped alongside any hostile payload, silently breaking jr's own
    /// intentional Active-column coloring. The BC therefore requires
    /// `format_active`'s styling to live on structural `comfy_table::Cell`
    /// attributes instead (via `active_cell`) — `format_active` itself must
    /// return the BARE glyph, no ANSI bytes embedded in the `String`,
    /// regardless of whether color is currently enabled.
    ///
    /// Deterministic without a real TTY: `colored`'s own suppression when
    /// stdout isn't a terminal (the default under `cargo test`, where
    /// stdout is captured) would make this assertion trivially true for
    /// the WRONG reason — not because `format_active` is structural, but
    /// because `colored` isn't emitting ANSI at all in this process.
    /// Forcing the override ON via `ForcedColorOverride` closes that gap:
    /// with color forced on, a non-structural `format_active` would embed
    /// ANSI bytes (`"\x1b[32m✓\x1b[0m"` / `"\x1b[31m✗\x1b[0m"`), so this
    /// test is a genuine regression guard against reintroducing that.
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

    /// This test validates the GENERAL TECHNIQUE BC-7.1.006 uses for
    /// `format_active`'s structural styling — `comfy_table::Cell` styling
    /// (`Cell::new(glyph).fg(Color::Green)`) survives even though the cell
    /// TEXT itself (the bare glyph `format_active` returns) passes through
    /// `sanitize_table_cell` unchanged. It exercises `comfy_table` directly
    /// rather than `active_cell`/`render_table_with_styles` (whose actual
    /// wiring is pinned by the `output::` unit tests and by
    /// `active_cell`'s own tests below), so it does not pin jr's own code
    /// — it's a documentation-style regression guard for the underlying
    /// `comfy_table` behavior this BC's design depends on.
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

    /// BC-7.1.006: `--no-color`/`NO_COLOR` must still suppress the Active
    /// column's structural coloring, even though `comfy_table`'s own
    /// `Table::should_style()` gate knows nothing about either — it's a
    /// pure TTY check. `active_cell` must consult
    /// `colored::control::SHOULD_COLORIZE.should_colorize()` (the same
    /// flag `main.rs` drives via `colored::control::set_override(false)`
    /// for `--no-color`/`NO_COLOR`) and fall back to a plain,
    /// uncolored `StyledCell` when it is false — regardless of what the
    /// `active` value would otherwise resolve to.
    #[test]
    fn test_bc_7_1_006_active_cell_no_color_override_suppresses_structural_color() {
        let _color = ForcedColorOverride::new(false);

        assert_eq!(
            active_cell(Some(true)),
            output::StyledCell::plain("✓"),
            "--no-color/NO_COLOR must suppress the Active column's \
             structural green styling, not just its ANSI-in-text form"
        );
        assert_eq!(
            active_cell(Some(false)),
            output::StyledCell::plain("✗"),
            "--no-color/NO_COLOR must suppress the Active column's \
             structural red styling, not just its ANSI-in-text form"
        );
        assert_eq!(active_cell(None), output::StyledCell::plain("—"));
    }

    /// Sibling of the suppression test above: with color forced ON, the
    /// Active column's structural styling IS applied — `active_cell` must
    /// not suppress unconditionally.
    #[test]
    fn test_bc_7_1_006_active_cell_colorizes_when_should_colorize_true() {
        let _color = ForcedColorOverride::new(true);

        assert_eq!(
            active_cell(Some(true)),
            output::StyledCell::colored("✓", Color::Green)
        );
        assert_eq!(
            active_cell(Some(false)),
            output::StyledCell::colored("✗", Color::Red)
        );
        assert_eq!(active_cell(None), output::StyledCell::plain("—"));
    }
}
