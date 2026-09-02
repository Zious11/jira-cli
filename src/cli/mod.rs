pub mod api;
pub mod assets;
pub mod auth;
pub mod board;
pub mod component;
pub mod field;
pub mod init;
pub mod issue;
pub mod project;
pub mod queue;
pub mod requesttype;
pub mod sprint;
pub mod team;
pub mod user;
pub mod worklog;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "jr", version, about = "A fast CLI for Jira Cloud")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Output format
    #[arg(long, global = true, default_value = "table")]
    pub output: OutputFormat,

    /// Override project key
    #[arg(long, global = true)]
    pub project: Option<String>,

    /// Disable colored output
    #[arg(long, global = true)]
    pub no_color: bool,

    /// Disable interactive prompts (auto-enabled when stdin is not a TTY)
    #[arg(long, global = true)]
    pub no_input: bool,

    /// verbose mode (method + URL only; use --verbose-bodies for full body inspection)
    #[arg(long, global = true)]
    pub verbose: bool,

    /// print full HTTP request/response bodies to stderr (PII warning emitted; use with care — see CLAUDE.md)
    #[arg(long, global = true)]
    pub verbose_bodies: bool,

    /// Override the active profile (precedence: this flag > JR_PROFILE > config > "default")
    #[arg(long, global = true)]
    pub profile: Option<String>,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    Table,
    Json,
}

#[derive(Subcommand)]
pub enum Command {
    /// Initialize jr configuration
    Init,
    /// Manage Assets/CMDB objects
    Assets {
        #[command(subcommand)]
        command: AssetsCommand,
    },
    /// Manage authentication
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
    /// Show current user info
    Me,
    /// Show valid issue types, priorities, and statuses for a project
    #[command(name = "project")]
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    /// Manage issues
    Issue {
        #[command(subcommand)]
        command: Box<IssueCommand>,
    },
    /// Manage boards
    Board {
        #[command(subcommand)]
        command: BoardCommand,
    },
    /// Manage sprints
    Sprint {
        #[command(subcommand)]
        command: SprintCommand,
    },
    /// Manage worklogs
    Worklog {
        #[command(subcommand)]
        command: WorklogCommand,
    },
    /// Manage teams
    Team {
        #[command(subcommand)]
        command: TeamCommand,
    },
    /// Manage users
    User {
        #[command(subcommand)]
        command: UserCommand,
    },
    /// Manage JSM queues
    Queue {
        #[command(subcommand)]
        command: QueueCommand,
    },
    /// Discover JSM request types and their fields
    #[command(name = "requesttype")]
    RequestType {
        #[command(subcommand)]
        command: RequestTypeCommand,
    },
    /// Manage project components
    Component {
        #[command(subcommand)]
        command: ComponentSubcommand,
    },
    /// Discover custom-field allowed options (issue #580)
    Field {
        #[command(subcommand)]
        command: FieldCommand,
    },
    /// Make a raw authenticated HTTP request to the Jira REST API.
    Api {
        /// API path (leading slash optional). Example: /rest/api/3/myself
        path: String,

        /// HTTP method
        #[arg(short = 'X', long, value_enum, ignore_case = true, default_value_t = api::HttpMethod::Get)]
        method: api::HttpMethod,

        /// Request body: inline JSON, @file to read from a file, or @- to read from stdin
        #[arg(short = 'd', long)]
        data: Option<String>,

        /// Custom header in "Key: Value" format (repeatable)
        #[arg(short = 'H', long = "header")]
        header: Vec<String>,
    },
    /// Generate shell completions
    Completion {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

#[derive(Subcommand)]
pub enum AssetsCommand {
    /// Search assets with AQL query
    Search {
        /// AQL query (e.g. "objectType = Client")
        query: String,
        /// Maximum number of results
        #[arg(long)]
        limit: Option<u32>,
        /// Include object attributes in output
        #[arg(long)]
        attributes: bool,
    },
    /// View asset details
    View {
        /// Object key (e.g. OBJ-1) or numeric ID
        key: String,
        /// Omit object attributes from output
        #[arg(long)]
        no_attributes: bool,
    },
    /// Show Jira issues connected to an asset
    Tickets {
        /// Object key (e.g. OBJ-1) or numeric ID
        key: String,
        /// Maximum number of tickets to show
        #[arg(long)]
        limit: Option<u32>,
        /// Show only open tickets (excludes Done status category)
        #[arg(long, conflicts_with = "status")]
        open: bool,
        /// Filter by status (partial match supported)
        #[arg(long, conflicts_with = "open")]
        status: Option<String>,
    },
    /// List object schemas in the workspace
    Schemas,
    /// List object types (all schemas or filtered)
    Types {
        /// Filter by schema (partial name match or exact ID)
        #[arg(long)]
        schema: Option<String>,
    },
    /// Show attributes for an object type
    Schema {
        /// Object type name (partial match supported)
        name: String,
        /// Filter by schema (partial name match or exact ID)
        #[arg(long)]
        schema: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum AuthCommand {
    /// Authenticate with Jira
    Login {
        /// Profile to log in to (creates it if absent). Defaults to active profile.
        #[arg(long)]
        profile: Option<String>,
        /// Jira instance URL (required when creating a new profile under --no-input).
        #[arg(long)]
        url: Option<String>,
        /// Use OAuth 2.0 instead of API token (requires your own OAuth app).
        /// Scope list is Atlassian's recommended classic set by default;
        /// override via `[profiles.<name>].oauth_scopes` in config.toml — see
        /// Configuration below.
        ///
        /// DEPRECATED (BC-1.2.049): retained as an accepted alias — a
        /// deprecation notice is printed to stderr in human-output mode.
        /// Prefer letting the interactive picker default to OAuth, or pass
        /// `--api-token` explicitly for the other mechanism.
        #[arg(long, conflicts_with = "api_token")]
        oauth: bool,
        /// Select the API-token mechanism directly, skipping the
        /// interactive OAuth-default picker (BC-1.2.050). Mutually
        /// exclusive with `--oauth`.
        #[arg(long, conflicts_with = "oauth")]
        api_token: bool,
        /// Jira email (API token flow). Prefer $JR_EMAIL over this flag.
        #[arg(long)]
        email: Option<String>,
        /// API token (API token flow). Prefer $JR_API_TOKEN over this flag — bare
        /// CLI args can leak via process lists (`ps`, audit logs).
        #[arg(long)]
        token: Option<String>,
        /// OAuth Client ID (OAuth flow). Prefer $JR_OAUTH_CLIENT_ID over this flag.
        #[arg(long)]
        client_id: Option<String>,
        /// OAuth Client Secret (OAuth flow). Prefer $JR_OAUTH_CLIENT_SECRET over
        /// this flag — bare CLI args can leak via process lists.
        #[arg(long)]
        client_secret: Option<String>,
        /// Cloud ID to use when multiple Atlassian orgs are accessible
        /// (disambiguates which site to target). Use this in scripts to select
        /// the correct org. Run `jr auth login --oauth` interactively first to
        /// see available org IDs and names.
        #[arg(long)]
        cloud_id: Option<String>,
    },
    /// Show authentication status
    Status {
        /// Profile to show status for. Defaults to active profile.
        #[arg(long)]
        profile: Option<String>,
    },
    /// Clear stored credentials and re-run the login flow.
    ///
    /// On macOS, run this after upgrading `jr` (e.g., `brew upgrade`, binary
    /// replacement). The legacy Keychain ACL is bound to the original binary's
    /// identity; this command deletes the entries so the new binary becomes
    /// the creator of fresh entries, avoiding repeated "allow access"
    /// prompts. See issue #207.
    Refresh {
        /// Profile to refresh credentials for. Defaults to active profile.
        #[arg(long)]
        profile: Option<String>,
        /// Use OAuth 2.0 instead of API token (matches `jr auth login --oauth`).
        ///
        /// DEPRECATED (BC-1.2.049): retained as an accepted alias, but has no
        /// effect on `auth refresh`'s mechanism selection (BC-1.2.051) — the
        /// profile's own stored `auth_method` is always used. A deprecation
        /// notice is printed to stderr in human-output mode.
        #[arg(long, conflicts_with = "api_token")]
        oauth: bool,
        /// Syntactically accepted for symmetry with `auth login`
        /// (BC-1.2.050); has no effect on `auth refresh`'s mechanism
        /// selection — the profile's own stored `auth_method` is always
        /// used. An informational stderr notice is printed in human-output
        /// mode. Mutually exclusive with `--oauth`.
        #[arg(long, conflicts_with = "oauth")]
        api_token: bool,
        /// Jira email (API token flow). Prefer $JR_EMAIL over this flag.
        #[arg(long)]
        email: Option<String>,
        /// API token (API token flow). Prefer $JR_API_TOKEN over this flag —
        /// bare CLI args can leak via process lists.
        #[arg(long)]
        token: Option<String>,
        /// OAuth Client ID (OAuth flow). Prefer $JR_OAUTH_CLIENT_ID over this flag.
        #[arg(long)]
        client_id: Option<String>,
        /// OAuth Client Secret (OAuth flow). Prefer $JR_OAUTH_CLIENT_SECRET over
        /// this flag — bare CLI args can leak via process lists.
        #[arg(long)]
        client_secret: Option<String>,
    },
    /// Set the default profile in config.toml.
    Switch {
        /// Profile name to make active. Must already exist in config.
        name: String,
    },
    /// List all configured profiles, marking the active one.
    List,
    /// Clear OAuth tokens for a profile (profile entry stays in config).
    /// Shared API-token credential is NEVER touched.
    Logout {
        /// Profile to log out of. Defaults to active profile.
        #[arg(long)]
        profile: Option<String>,
    },
    /// Permanently delete a profile (config + cache + per-profile OAuth tokens).
    /// Shared credentials are NEVER touched.
    Remove {
        /// Profile name to remove. Cannot be the active profile —
        /// switch first with `jr auth switch`.
        name: String,
    },
}

#[derive(Subcommand)]
pub enum IssueCommand {
    /// List issues
    List {
        /// JQL query
        #[arg(long)]
        jql: Option<String>,
        /// Filter by status
        #[arg(long)]
        status: Option<String>,
        /// Filter by team
        #[arg(long)]
        team: Option<String>,
        /// Maximum number of results
        #[arg(long)]
        limit: Option<u32>,
        /// Fetch all results (no default limit)
        #[arg(long, conflicts_with = "limit")]
        all: bool,
        /// Filter by assignee ("me" for current user, or a name to search)
        #[arg(long)]
        assignee: Option<String>,
        /// Filter by reporter ("me" for current user, or a name to search)
        #[arg(long)]
        reporter: Option<String>,
        /// Show issues created within duration (e.g., 7d, 4w, 2M)
        #[arg(long)]
        recent: Option<String>,
        /// Show issues updated within duration (e.g., 7d, 4w, 2M)
        #[arg(long)]
        updated_recent: Option<String>,
        /// Show only open issues (excludes Done status category)
        #[arg(long, conflicts_with = "status")]
        open: bool,
        /// Show story points column
        #[arg(long)]
        points: bool,
        /// Show linked assets column
        #[arg(long)]
        assets: bool,
        /// Show due date column
        #[arg(long)]
        duedate: bool,
        /// Filter by linked asset object key (e.g., CUST-5)
        #[arg(long)]
        asset: Option<String>,
        /// Filter by component name (repeatable, OR-combined). Prefix forms:
        /// `not:<NAME>` excludes (issues with no component are still included),
        /// `none` matches issues with zero components (must be the only
        /// occurrence), `all:<N1>,<N2>` requires every listed component
        /// (AND-combined; at most one `all:` occurrence). See BC-2.1.018..022.
        #[arg(long = "component")]
        component: Vec<String>,
        /// Show issues created on or after this date (YYYY-MM-DD)
        #[arg(long, conflicts_with = "recent")]
        created_after: Option<String>,
        /// Show issues created on or before this date (YYYY-MM-DD)
        #[arg(long)]
        created_before: Option<String>,
        /// Show issues updated on or after this date (YYYY-MM-DD)
        #[arg(long, conflicts_with = "updated_recent")]
        updated_after: Option<String>,
        /// Show issues updated on or before this date (YYYY-MM-DD)
        #[arg(long)]
        updated_before: Option<String>,
        /// Comma-separated list of fields to request from Jira (e.g.
        /// "summary,status,comment"), REPLACING the default field set
        /// (BASE_ISSUE_FIELDS plus any --points/--assets/--duedate extras)
        /// rather than unioning with it (DEC-298). Requires --output json;
        /// combined with table mode (default or --output table) exits 64
        /// pre-HTTP. See BC-2.2.033.
        #[arg(long)]
        fields: Option<String>,
        /// Sort results by a field (e.g. "updated:desc", "key:asc"). Overrides
        /// the default/board-driven ordering in every JQL composition branch
        /// (including `--jql`'s own ORDER BY and scrum/kanban board rank
        /// ordering); appends `, key ASC` as a stable secondary sort unless
        /// the field is `key` itself. Field name is passed through to Jira
        /// unvalidated -- the same trust posture as `--jql`. See BC-2.1.024
        /// and BC-2.1.025.
        #[arg(long)]
        sort: Option<String>,
    },
    /// Create a new issue
    Create {
        /// Project key
        #[arg(short, long)]
        project: Option<String>,
        /// Issue type
        #[arg(short = 't', long = "type")]
        issue_type: Option<String>,
        /// Summary
        #[arg(short, long, allow_hyphen_values = true)]
        summary: Option<String>,
        /// Description
        #[arg(
            short,
            long,
            allow_hyphen_values = true,
            conflicts_with = "description_stdin"
        )]
        description: Option<String>,
        /// Read description from stdin (for piping)
        #[arg(long, conflicts_with = "description")]
        description_stdin: bool,
        /// Priority
        #[arg(long)]
        priority: Option<String>,
        /// Labels (can be specified multiple times)
        #[arg(long)]
        label: Vec<String>,
        /// Set initial components (repeatable). Resolved via the project's
        /// component list (BC-3.4.025), the same resolver as `jr component`
        /// and `issue list --component`. No add:/remove: prefix grammar on
        /// create — a literal `add:X` is resolved as-is and 400s as an
        /// unknown name (BC-3.4.024). Cannot be combined with --request-type
        /// (BC-3.4.024 Postcondition 3).
        #[arg(long = "component")]
        component: Vec<String>,
        /// Team assignment
        #[arg(long)]
        team: Option<String>,
        /// Story points
        #[arg(long)]
        points: Option<f64>,
        /// Interpret description as Markdown
        #[arg(long)]
        markdown: bool,
        /// Parent issue key (e.g., for subtasks or stories under epics)
        #[arg(long)]
        parent: Option<String>,
        /// Assign to user (name/email, or "me" for self)
        #[arg(long, conflicts_with = "account_id")]
        to: Option<String>,
        /// Assign to this Jira accountId directly (bypasses name search)
        #[arg(long, conflicts_with = "to")]
        account_id: Option<String>,
        /// JSM request type (name or numeric ID). When set, dispatches to
        /// POST /rest/servicedeskapi/request instead of POST /rest/api/3/issue.
        /// The project must be a Jira Service Management project.
        #[arg(long = "request-type")]
        request_type: Option<String>,
        /// Set a custom field as NAME=VALUE, or NAME:kind=VALUE (repeatable).
        /// The first '=' splits; subsequent '=' characters are part of the value.
        /// Duplicate keys use the last value provided. On the platform (non-JSM)
        /// path, resolves against the project's Create screen (createmeta); with
        /// --request-type set, resolves against the JSM request type's fields.
        #[arg(long = "field", action = clap::ArgAction::Append)]
        field: Vec<String>,
        /// Create the request on behalf of this accountId (JSM only; requires
        /// --request-type).
        /// Maps to the top-level `raiseOnBehalfOf` field in the request body.
        #[arg(long = "on-behalf-of")]
        on_behalf_of: Option<String>,
    },
    /// View issue details
    View {
        /// Issue key (e.g., FOO-123)
        key: String,
        /// Comma-separated list of fields to request from Jira (e.g.
        /// "summary,comment"), REPLACING the default field set rather than
        /// unioning with it (DEC-298). Requires --output json; combined with
        /// table mode (default or --output table) exits 64 pre-HTTP. See
        /// BC-2.3.041.
        #[arg(long)]
        fields: Option<String>,
    },
    /// Edit issue fields
    Edit {
        /// Issue keys (positional; omit when using --jql). Mutually exclusive with --jql.
        /// Non-`--component` bulk edits are capped at 1000 keys per call (Atlassian Bulk
        /// API limit, enforced in `handle_edit`). `--component` bulk edits (S-605-2,
        /// BC-3.4.023 Postcondition 6) chunk internally into <=1000-key POSTs, so the CLI
        /// surface allows a much larger key set for that flag — widened from the prior
        /// `0..=1001` cap so a >1000-key `--component` invocation can reach the handler.
        #[arg(num_args = 0..=10000, conflicts_with = "jql")]
        keys: Vec<String>,
        /// JQL query to select issues for bulk edit. Mutually exclusive with positional keys.
        #[arg(long, conflicts_with = "keys")]
        jql: Option<String>,
        /// Maximum number of issues to match via --jql (default 50, hard ceiling 1000
        /// for non-`--component` bulk edits; `--component` bulk edits chunk internally
        /// into <=1000-key POSTs (S-605-2, BC-3.4.023 Postcondition 6) and accept up to
        /// 10000). Requires --jql; cannot be used with positional keys. If the JQL
        /// match count exceeds this value, the command errors without mutating.
        ///
        /// Values above 100 trigger cursor pagination on /rest/api/3/search/jql (Jira
        /// caps maxResults at 100 per page), so a large --max triggers multiple search
        /// requests before the bulk call. Use the smallest --max that fits your workflow.
        ///
        /// The clap-level range here is widened to 1..=10000 so a >1000 `--component`
        /// invocation can reach the handler at all; `handle_edit` enforces the tighter
        /// 1000 ceiling at runtime for every OTHER bulk field flag (it cannot see
        /// --component's presence from a value_parser alone).
        #[arg(long, value_parser = clap::value_parser!(u32).range(1..=10_000))]
        max: Option<u32>,
        /// Skip the interactive confirmation prompt for large JQL match sets.
        #[arg(long)]
        yes: bool,
        /// Preview the planned changes without making any HTTP mutations.
        /// For --jql, the search IS executed (read-only); for positional keys, no HTTP calls.
        #[arg(long)]
        dry_run: bool,
        /// New summary
        #[arg(long, allow_hyphen_values = true)]
        summary: Option<String>,
        /// New issue type
        #[arg(long = "type")]
        issue_type: Option<String>,
        /// New priority
        #[arg(long)]
        priority: Option<String>,
        /// Add or remove labels (e.g., --label add:backend --label remove:frontend)
        #[arg(long)]
        label: Vec<String>,
        /// Add or remove components (e.g., --component add:backend --component
        /// remove:frontend). Bare values (no prefix) are treated as ADD
        /// (BC-3.4.022). A single key uses the native `update`-verb PUT path;
        /// 2+ keys (positional or --jql-resolved) route to a dedicated bulk
        /// multiselectComponents path (BC-3.4.023, S-605-2), chunked
        /// internally into <=1000-key POSTs. On 2+ keys, --component cannot
        /// be combined with --summary/--priority/--type in the same call
        /// (the bulk path has no way to also carry those fields). Cannot be
        /// combined with --label on the same call at any key count
        /// (BC-3.4.020 amendment).
        #[arg(long = "component")]
        component: Vec<String>,
        /// Team assignment
        #[arg(long)]
        team: Option<String>,
        /// Story points
        #[arg(long, conflicts_with = "no_points")]
        points: Option<f64>,
        /// Clear story points
        #[arg(long, conflicts_with = "points")]
        no_points: bool,
        /// Parent issue key
        #[arg(long, conflicts_with = "no_parent")]
        parent: Option<String>,
        /// Clear the issue's parent
        #[arg(long, conflicts_with = "parent")]
        no_parent: bool,
        /// Description
        #[arg(
            short,
            long,
            allow_hyphen_values = true,
            conflicts_with = "description_stdin"
        )]
        description: Option<String>,
        /// Read description from stdin (for piping)
        #[arg(long, conflicts_with = "description")]
        description_stdin: bool,
        /// Interpret description as Markdown
        #[arg(long)]
        markdown: bool,
        /// Arbitrary custom field values as NAME=VALUE pairs (repeatable).
        /// The first '=' splits name from value; subsequent '=' are part of the value.
        /// Duplicate keys use the last value provided. Single-key path only (rejected
        /// in bulk-edit context). See also: CLAUDE.md Gotchas — `--field` on issue edit.
        #[arg(long = "field", action = clap::ArgAction::Append)]
        field: Vec<String>,
    },
    /// Transition one or more issues to a new status
    ///
    /// Single-key legacy form:  jr issue move FOO-100 Done
    /// Multi-key form:          jr issue move FOO-100 FOO-101 FOO-102 --to Done
    ///
    /// For multi-key (2+ keys), `--to` is required. For single-key, the
    /// trailing positional is the target status (backward compatible).
    Move {
        /// Issue keys (legacy single-key: KEY STATUS; multi-key: KEY1 KEY2 ... --to STATUS)
        #[arg(required = true, num_args = 1..=1001)]
        keys: Vec<String>,
        /// Target status for multi-key form (required when 2+ keys are given)
        #[arg(long)]
        to: Option<String>,
        /// Set resolution atomically with the transition (e.g. "Fixed"). Many
        /// JSM workflows require this; run `jr issue resolutions` to discover
        /// valid values. (Single-key only; ignored in multi-key bulk form.)
        #[arg(long, conflicts_with = "no_resolution")]
        resolution: Option<String>,
        /// Explicit opt-out from proactive resolution enforcement. Use when
        /// moving to a done-category status where a null resolution is genuinely
        /// intentional (e.g., "Won't Do" automation paths). Mutually exclusive
        /// with --resolution. No effect on non-done-category transitions.
        /// (BC-3.2.013; ADR-0015 §7)
        #[arg(long, conflicts_with = "resolution")]
        no_resolution: bool,
    },
    /// List available transitions without performing one
    Transitions {
        /// Issue key
        key: String,
    },
    /// List the resolution values defined on this Jira instance. Cached
    /// for 7 days; use --refresh to bypass the cache.
    Resolutions {
        /// Bypass the local cache and re-fetch from the server.
        #[arg(long)]
        refresh: bool,
    },
    /// Assign issue
    Assign {
        /// Issue key
        key: String,
        /// Assign to this user (name/email, or "me" for self; omit to assign to self)
        #[arg(long, conflicts_with_all = ["account_id", "unassign"])]
        to: Option<String>,
        /// Assign to this Jira accountId directly (bypasses name search)
        #[arg(long, conflicts_with_all = ["to", "unassign"])]
        account_id: Option<String>,
        /// Remove assignee
        #[arg(long, conflicts_with_all = ["to", "account_id"])]
        unassign: bool,
    },
    /// Comment operations: add, delete, edit, view.
    /// To list all comments on an issue, use `jr issue comments`.
    Comment {
        #[command(subcommand)]
        command: CommentSubcommand,
    },
    /// List comments on an issue
    Comments {
        /// Issue key (e.g., FOO-123)
        key: String,
        /// Maximum number of comments to return
        #[arg(long)]
        limit: Option<u32>,
    },
    /// Show an issue's audit changelog (status/field changes)
    Changelog {
        /// Issue key (e.g., FOO-123)
        key: String,
        /// Maximum number of rows (default 30). Applies post-filter.
        #[arg(long, conflicts_with = "all")]
        limit: Option<u32>,
        /// Show all rows (disable the default 30-row limit)
        #[arg(long, conflicts_with = "limit")]
        all: bool,
        /// Filter by field name; repeatable (case-insensitive substring)
        #[arg(long = "field")]
        field: Vec<String>,
        /// Filter by author ("me" for current user, or a name/accountId)
        ///
        /// "me" is reserved and resolves to the current user. AccountIds
        /// (values containing ':' or ≥12 characters of `[A-Za-z0-9_-]`
        /// that include at least one digit) are matched exactly; other
        /// values match as a case-insensitive substring of displayName
        /// or accountId.
        #[arg(long)]
        author: Option<String>,
        /// Render oldest-first instead of default newest-first
        #[arg(long)]
        reverse: bool,
    },
    /// Open issue in browser
    Open {
        /// Issue key
        key: String,
        /// Print URL instead of opening browser (for scripting/AI agents)
        #[arg(long)]
        url_only: bool,
    },
    /// Link two issues
    ///
    /// Creates a directional link (e.g., "blocks", "is blocked by") between
    /// `<KEY1>` and `<KEY2>`. The link type defaults to "Relates"; pass
    /// `--type <NAME>` to use any other type from your Jira instance.
    ///
    /// See also: `jr issue unlink` to remove a link, `jr issue link-types` to
    /// list available link type names.
    Link {
        /// First issue key (outward — e.g., the issue that "blocks")
        key1: String,
        /// Second issue key (inward — e.g., the issue that "is blocked by")
        key2: String,
        /// Link type name (partial match supported, default: "Relates")
        #[arg(long, default_value = "Relates")]
        r#type: String,
    },
    /// Remove link(s) between two issues
    ///
    /// Removes one or more links between `<KEY1>` and `<KEY2>`. If `--type` is
    /// omitted, ALL links between the pair are removed. Pass `--type <NAME>` to
    /// scope deletion to a specific link type.
    ///
    /// See also: `jr issue link` to create a link, `jr issue link-types` to
    /// list available link type names.
    Unlink {
        /// First issue key
        key1: String,
        /// Second issue key
        key2: String,
        /// Only remove links of this type (removes all if omitted)
        #[arg(long)]
        r#type: Option<String>,
    },
    /// Link a Confluence page or arbitrary web URL to an issue as a remote link.
    ///
    /// Renders under the issue's "Web links" (or "Confluence pages") panel in
    /// Jira's UI. Jira decides which panel based on its own app-integration
    /// metadata — this command creates a plain remote link and lets Jira sort
    /// it into the right panel.
    RemoteLink {
        /// Issue key (e.g. PROJ-123).
        key: String,

        /// URL to link to.
        #[arg(long)]
        url: String,

        /// Label shown in the Jira UI. Defaults to the URL when omitted.
        #[arg(long, allow_hyphen_values = true)]
        title: Option<String>,
    },
    /// List available link types
    LinkTypes,
    /// Show assets linked to an issue
    Assets {
        /// Issue key (e.g., FOO-123)
        key: String,
    },
    /// Attachment operations: list, download, upload, delete. (S-576-1..4)
    Attachment {
        #[command(subcommand)]
        command: AttachmentSubcommand,
    },
}

/// Subcommands for `jr issue comment`.
#[derive(Subcommand)]
pub enum CommentSubcommand {
    /// Add a comment (canonical form; replaces the old flat `jr issue comment KEY text`)
    Add {
        /// Issue key
        key: String,
        /// Comment text (leading-dash values accepted)
        #[arg(allow_hyphen_values = true)]
        message: Option<String>,
        /// Interpret input as Markdown
        #[arg(long)]
        markdown: bool,
        /// Read comment from file
        #[arg(long)]
        file: Option<String>,
        /// Read comment from stdin (for piping)
        #[arg(long)]
        stdin: bool,
        /// Mark comment as internal — agent-only, not visible to the customer on the JSM
        /// portal. Without this flag the comment is a public reply (the default). No-op on
        /// standard (non-JSM) projects, where Jira ignores the `sd.public.comment` property.
        #[arg(long)]
        internal: bool,
    },
    /// Delete a comment by ID — requires --yes or interactive confirmation
    Delete {
        /// Issue key
        key: String,
        /// Comment ID to delete
        #[arg(long)]
        id: String,
        /// Skip interactive confirmation
        #[arg(long)]
        yes: bool,
    },
    /// Edit a comment body (optionally set visibility)
    Edit {
        /// Issue key
        key: String,
        /// New comment body text (leading-dash values accepted)
        #[arg(allow_hyphen_values = true, conflicts_with_all = ["file", "stdin"])]
        text: Option<String>,
        /// Comment ID to edit
        #[arg(long)]
        id: String,
        /// Read new body from file
        #[arg(long, conflicts_with_all = ["stdin", "text"])]
        file: Option<String>,
        /// Read new body from stdin
        #[arg(long, conflicts_with_all = ["file", "text"])]
        stdin: bool,
        /// Interpret body as Markdown
        #[arg(long)]
        markdown: bool,
        /// Mark comment as internal (agent-only visibility)
        #[arg(long, conflicts_with = "public")]
        internal: bool,
        /// Mark comment as public (visible to customers on JSM portal)
        #[arg(long, conflicts_with = "internal")]
        public: bool,
        /// Skip interactive confirmation
        #[arg(long)]
        yes: bool,
    },
    /// View a single comment by ID
    View {
        /// Issue key
        key: String,
        /// Comment ID to view
        #[arg(long)]
        id: String,
    },
}

/// Subcommands for `jr issue attachment`. (S-576-1)
///
/// Defined here (in `src/cli/mod.rs`) — NOT in `src/cli/issue/attachments.rs`.
/// S-576-2, S-576-3, S-576-4, and S-576-5 add variants to this enum additively.
///
/// **Additive-only coordination:** each story appends its variant and dispatch arm;
/// never remove or reorder a sibling story's variant or arm (P26-002).
#[derive(Subcommand)]
pub enum AttachmentSubcommand {
    /// List attachments on an issue (table or JSON; client-side filters)
    List {
        /// Issue key (e.g., FOO-123)
        key: String,
        /// Client-side filter: `mime=<glob>`, `name=<glob>`, or `size-max=<bytes>`.
        /// Repeatable; multiple filters combine with AND semantics.
        #[arg(long = "filter")]
        filter: Vec<String>,
    },

    /// Download one or more attachments from an issue (S-576-2; BC-2.7.007..012).
    ///
    /// Requires exactly one selector: `--id`, `--all`, or `--newest N`.
    #[command(
        group(clap::ArgGroup::new("selector").required(true).args(["id", "all", "newest"])),
        group(clap::ArgGroup::new("batch").args(["all", "newest"])),
    )]
    Download {
        /// Issue key (e.g., FOO-123)
        key: String,

        /// Attachment ID to download (numeric).
        /// Mutually exclusive with `--all` and `--newest` (via `selector` group).
        #[arg(long)]
        id: Option<String>,

        /// Download all attachments from the issue to `--out-dir` (or cwd when omitted).
        /// Mutually exclusive with `--id` and `--newest` (via `selector` group).
        #[arg(long)]
        all: bool,

        /// Download the N most-recent attachments by `created` descending.
        /// `--filter` predicates (if any) are applied BEFORE this truncation — the
        /// surviving set is sorted by `created` descending (most recent first),
        /// then truncated to the first N.
        /// Accepts negative integers — N ≤ 0 is rejected in the handler (exit 64,
        /// `--newest requires a positive integer.`; EC-2.7.009-1; `allow_negative_numbers`
        /// lets clap accept them so the handler can emit the canonical message).
        /// Mutually exclusive with `--id` and `--all` (via `selector` group).
        #[arg(long, allow_negative_numbers = true)]
        newest: Option<i64>,

        /// Output path for a single-file download (requires `--id`; not valid with
        /// `--all` or `--newest`; EC-2.7.007-9 ~769).
        #[arg(long, requires = "id", conflicts_with_all = ["all", "newest"])]
        out: Option<std::path::PathBuf>,

        /// Output directory for batch downloads.
        /// Requires the `batch` group (`--all` or `--newest`; EC-2.7.008-9 ~812).
        /// Conflicts with `--id`. Files land as
        /// `<40-char-SHA-1-of-the-attachment-id>_<sanitized-filename>` — the
        /// on-disk name is NOT predictable from `list` output; recover it by
        /// parsing the `path` field of this command's JSON manifest.
        #[arg(long = "out-dir", requires = "batch", conflicts_with = "id")]
        out_dir: Option<std::path::PathBuf>,

        /// Client-side filter: `mime=<glob>`, `name=<glob>`, or `size-max=<bytes>`.
        /// Repeatable; AND semantics. Conflicts with `--id` (EC-2.7.007-10 ~770).
        #[arg(long = "filter", conflicts_with = "id")]
        filter: Vec<String>,

        /// Overwrite existing output files without error. Single-`--id`: bypasses
        /// the `--out` collision check (EC-2.7.007-12, SEC-576-010). Batch (`--all`
        /// / `--newest`): silently overwrites on filename collision (BC-2.7.008).
        #[arg(long)]
        force: bool,
    },

    /// Upload one or more files as attachments to a Jira issue (S-576-3; BC-3.9.001..020).
    ///
    /// Sends a single `multipart/form-data` POST with `X-Atlassian-Token: no-check`
    /// (BC-3.9.001). Multiple files are sent as separate `file`-named parts in one
    /// request (EC-3.9.001-2). stdin `-` as FILE → exit 64 before any HTTP call
    /// (EC-3.9.001-6).
    Upload {
        /// Issue key (e.g., FOO-123)
        key: String,

        /// File path(s) to upload. Repeatable. stdin `-` is rejected with exit 64
        /// (EC-3.9.001-6 canonical). Bare `-` passes through clap without
        /// `allow_hyphen_values`; that flag is intentionally absent here to
        /// prevent greedy consumption of `--output` and other trailing flags.
        #[arg(required = true, num_args = 1..)]
        file: Vec<std::path::PathBuf>,

        /// Delete existing same-filename attachments before uploading (BC-3.9.017).
        /// Requires interactive confirmation unless `--yes` is also supplied.
        /// Multiple same-filename attachments are ALL deleted (JRACLOUD-96384).
        #[arg(long)]
        replace_existing: bool,

        /// Skip the `--replace-existing` confirmation gate (non-interactive bypass;
        /// BC-3.9.014 consumer 2). No-op when `--replace-existing` is absent.
        #[arg(long)]
        yes: bool,

        /// Preview the upload without issuing any HTTP mutations (BC-3.9.020 path-c).
        /// Requires `--replace-existing` — exit 2 at parse time without it (EC-3.9.020-6).
        #[arg(long, requires = "replace_existing")]
        dry_run: bool,

        /// Mark the upload as customer-visible (public) on the JSM portal (BC-3.9.003).
        /// Routes through the servicedeskapi two-step flow. Requires a JSM project;
        /// exits 64 on non-JSM issues. Conflicts with --internal.
        #[arg(long, conflicts_with = "internal")]
        public: bool,

        /// Mark the upload as internal (agent-only) on JSM (BC-3.9.004).
        /// Routes through the servicedeskapi two-step flow on JSM issues.
        /// Silent no-op on non-JSM issues (falls through to platform upload). Conflicts with --public.
        #[arg(long, conflicts_with = "public")]
        internal: bool,
    },

    /// Delete one or more attachments by ID (S-576-4; BC-3.9.008..020).
    ///
    /// Three forms:
    ///   (1) Single-AID: `jr issue attachment delete <AID>` — confirmation gate (BC-3.9.015).
    ///   (2) Multi-AID:  `jr issue attachment delete <AID1> <AID2>... --yes` — bulk, no gate.
    ///   (3) Issue+age:  `jr issue attachment delete --issue <KEY> --older-than <DUR> --yes`.
    ///
    /// Clap constraints (EC-3.9.016-4/5/9/10):
    ///   - Positional `<AID>...` conflicts with `--issue` and `--older-than`.
    ///   - `--issue` requires `--older-than`; `--older-than` requires `--issue`.
    ///   - Must supply at least one form; bare `delete` (no AID, no flags) → exit 2.
    #[command(
        group(
            clap::ArgGroup::new("delete_target")
                .required(true)
                .multiple(true)
                .args(["aids", "issue"])
        ),
    )]
    Delete {
        /// Attachment IDs to delete (numeric; repeatable).
        /// Conflicts with `--issue` and `--older-than` (EC-3.9.016-4).
        #[arg(conflicts_with_all = ["issue", "older_than"])]
        aids: Vec<String>,

        /// Issue key for age-based bulk delete.
        /// Requires `--older-than`; conflicts with positional AIDs (EC-3.9.016-9).
        #[arg(long, conflicts_with = "aids", requires = "older_than")]
        issue: Option<String>,

        /// Delete attachments older than this duration (e.g. 30d, 2w, 1h).
        /// Requires `--issue`; conflicts with positional AIDs (EC-3.9.016-5).
        #[arg(long, conflicts_with = "aids", requires = "issue")]
        older_than: Option<String>,

        /// Bypass the single-AID confirmation gate; required for bulk delete (BC-3.9.016).
        #[arg(long)]
        yes: bool,

        /// Preview the delete without issuing any HTTP DELETEs (BC-3.9.020 EC-3.9.020-1/2/3).
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand)]
pub enum ProjectCommand {
    /// List accessible projects
    List {
        /// Filter by project type (software, service_desk, business)
        #[arg(long = "type")]
        project_type: Option<String>,
        /// Maximum number of results (default: 50)
        #[arg(long)]
        limit: Option<u32>,
        /// Fetch all projects (paginate through all pages)
        #[arg(long, conflicts_with = "limit")]
        all: bool,
    },
    /// Show valid issue types, priorities, and statuses
    Fields,
}

#[derive(Subcommand)]
pub enum BoardCommand {
    /// List boards
    List {
        /// Filter by board type
        #[arg(long = "type", value_parser = clap::builder::PossibleValuesParser::new(["scrum", "kanban"]))]
        board_type: Option<String>,
    },
    /// View current board issues
    View {
        /// Board ID (overrides board_id in .jr.toml)
        #[arg(long)]
        board: Option<u64>,
        /// Maximum number of issues to return
        #[arg(long)]
        limit: Option<u32>,
        /// Fetch all results (no default limit)
        #[arg(long, conflicts_with = "limit")]
        all: bool,
    },
}

#[derive(Subcommand)]
pub enum SprintCommand {
    /// List sprints
    List {
        /// Board ID (overrides board_id in .jr.toml)
        #[arg(long)]
        board: Option<u64>,
    },
    /// Show current sprint issues
    Current {
        /// Board ID (overrides board_id in .jr.toml)
        #[arg(long)]
        board: Option<u64>,
        /// Maximum number of issues to return
        #[arg(long)]
        limit: Option<u32>,
        /// Fetch all results (no default limit)
        #[arg(long, conflicts_with = "limit")]
        all: bool,
    },
    /// Add issues to a sprint
    Add {
        /// Sprint ID (from `jr sprint list`)
        #[arg(long, required_unless_present = "current")]
        sprint: Option<u64>,
        /// Use the active sprint instead of specifying an ID
        #[arg(long, conflicts_with = "sprint")]
        current: bool,
        /// Issue keys to add (e.g. FOO-1 FOO-2)
        #[arg(required = true, num_args = 1..)]
        issues: Vec<String>,
        /// Board ID (used with --current to resolve the active sprint)
        #[arg(long)]
        board: Option<u64>,
    },
    /// Remove issues from sprint (moves to backlog)
    Remove {
        /// Issue keys to remove (e.g. FOO-1 FOO-2)
        #[arg(required = true, num_args = 1..)]
        issues: Vec<String>,
    },
}

#[derive(Subcommand)]
pub enum TeamCommand {
    /// List available teams
    List {
        /// Force refresh from API, ignoring cache
        #[arg(long)]
        refresh: bool,
    },
}

#[derive(Subcommand)]
pub enum UserCommand {
    /// Search for users by display name or email
    ///
    /// Results depend on the "Browse users and groups" global permission.
    /// Empty results may indicate either no matches or missing permission.
    /// Email is hidden when the target user's privacy settings opt out.
    Search {
        /// Search string (matches displayName and emailAddress substrings)
        query: String,
        /// Cap the number of results shown (default 30). Applies to both
        /// table rows and JSON array length; does not reduce the API fetch.
        #[arg(long)]
        limit: Option<u32>,
        /// Fetch all matching users by paginating through every API page
        /// (up to Jira's documented 1000-user hard cap). Overrides the
        /// default local cap.
        #[arg(long, conflicts_with = "limit")]
        all: bool,
    },
    /// List users assignable to a project
    ///
    /// Results depend on the "Browse users and groups" global permission.
    List {
        /// Project key (e.g., FOO)
        #[arg(long, short = 'p')]
        project: String,
        /// Cap the number of results shown (default 30). Applies to both
        /// table rows and JSON array length; does not reduce the API fetch.
        #[arg(long)]
        limit: Option<u32>,
        /// Fetch all assignable users by paginating through every API page
        /// (up to Jira's documented 1000-user hard cap). Overrides the
        /// default local cap.
        #[arg(long, conflicts_with = "limit")]
        all: bool,
    },
    /// Look up a user by accountId
    ///
    /// Resolves an accountId to displayName, email (when visible), and
    /// active status. Use this when you have an accountId and need the
    /// human-readable identity.
    View {
        /// Atlassian accountId
        account_id: String,
    },
}

#[derive(Subcommand)]
pub enum WorklogCommand {
    /// Log time on an issue
    Add {
        /// Issue key
        key: String,
        /// Duration (e.g., 2h, 1h30m, 1d)
        duration: String,
        /// Comment
        #[arg(short, long, allow_hyphen_values = true)]
        message: Option<String>,
    },
    /// List worklogs on an issue
    List {
        /// Issue key
        key: String,
    },
}

#[derive(Subcommand)]
pub enum QueueCommand {
    /// List queues for the service desk
    List,
    /// View issues in a queue
    View {
        /// Queue name (partial match supported)
        name: Option<String>,
        /// Queue ID (use if name is ambiguous)
        #[arg(long)]
        id: Option<String>,
        /// Maximum number of issues to return
        #[arg(long)]
        limit: Option<u32>,
    },
}

#[derive(Subcommand)]
pub enum RequestTypeCommand {
    /// List request types for the current project's service desk
    List {
        /// Filter results by name/description substring (server-side via searchQuery)
        #[arg(long)]
        search: Option<String>,
    },
    /// Show fields for a specific request type
    Fields {
        /// Request type name (partial match supported) OR numeric ID
        name_or_id: String,
    },
}

/// `jr field` subcommands (issue #580, BC-X.14.001..004).
#[derive(Subcommand)]
pub enum FieldCommand {
    /// Enumerate a custom field's allowed options
    ///
    /// Exactly one of `--type`, `--request-type`, `--issue` selects the
    /// enumeration mode; `--project` is a companion flag whose role
    /// (required-or-defaulted / optional / ignored) depends on the selected
    /// mode. See ADR-0019 §1 / BC-X.14.001.
    Options {
        /// `customfield_NNNNN` literal, or a human field name resolved via
        /// `list_fields()` + `partial_match`
        field: String,

        /// M2: enumerate via project+issue-type createmeta. Requires a
        /// resolvable `--project` (explicit flag or profile/config default).
        #[arg(long = "type")]
        r#type: Option<String>,

        /// M3: enumerate via JSM request-type fields. `--project` is an
        /// optional companion naming the service-desk project explicitly.
        #[arg(long = "request-type")]
        request_type: Option<String>,

        /// M1: enumerate via an existing issue's editmeta. `--project` is
        /// not consulted (the issue key alone supplies project context).
        #[arg(long)]
        issue: Option<String>,

        /// Companion project override — required-or-defaulted for `--type`,
        /// optional for `--request-type`, ignored for `--issue`.
        #[arg(long)]
        project: Option<String>,

        /// Client-side, case-insensitive substring filter against id/label
        #[arg(long)]
        value: Option<String>,
    },
}

/// Assignee-type policy for issues filed against a component.
///
/// Maps to Jira's `assigneeType` field on the component resource
/// (BC-8.1.005 — component create only; `component edit` has no `--assignee-type` flag).
#[derive(clap::ValueEnum, Clone, Debug)]
#[clap(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AssigneeType {
    /// Use the component lead as the default assignee.
    ComponentLead,
    /// Use the project lead as the default assignee.
    ProjectLead,
    /// Leave issues unassigned by default.
    Unassigned,
    /// Inherit the project's default assignee policy.
    ProjectDefault,
}

/// Subcommands for `jr component`.
///
/// Only `List` is implemented in S-604-1.  Create, edit, delete, and rename
/// subcommands land in subsequent stories (S-604-2, S-604-3, S-608-1) and are
/// added here additively — never remove or reorder a sibling variant.
#[derive(Subcommand)]
pub enum ComponentSubcommand {
    /// List components for a project
    List {
        /// Project key (overrides the configured default project).
        /// Required when no project is configured in `.jr.toml`.
        #[arg(long)]
        project: Option<String>,
        /// Enrich each component row with its related issue count.
        /// Issues one extra HTTP call per component (N+1). BC-8.1.003.
        #[arg(long)]
        counts: bool,
    },
    /// Create a new component in a project (BC-8.1.005 + BC-8.1.006 for --lead)
    Create {
        /// Project key. Required (no `.jr.toml` config fallback — BC-8.1.004 +
        /// BC-8.1.005); may be supplied here OR via the global `--project`
        /// flag (F5-A-L2 — clap's local-required-arg check does not see a
        /// value supplied only in the global position, so `handle_create`
        /// merges the two and enforces presence itself, exit 64).
        #[arg(long)]
        project: Option<String>,
        /// Component name (BC-8.1.005).
        /// Leading-dash values accepted (e.g. `-legacy`).
        #[arg(allow_hyphen_values = true)]
        name: String,
        /// Component description (leading-dash values accepted).
        #[arg(long, allow_hyphen_values = true)]
        description: Option<String>,
        /// Component lead: account ID, display-name substring, or email
        /// (resolved via `search_assignable_users_by_project`; BC-8.1.006).
        #[arg(long)]
        lead: Option<String>,
        /// Default assignee policy for issues in this component (BC-8.1.005).
        #[arg(long)]
        assignee_type: Option<AssigneeType>,
    },
    /// Edit an existing component's fields (BC-8.1.007)
    Edit {
        /// Component name (partial match) or numeric ID (BC-8.1.007 + BC-8.1.008 + BC-8.4.001).
        /// Leading-dash names accepted (e.g. `-legacy`).
        #[arg(allow_hyphen_values = true)]
        name_or_id: String,
        /// Project key (required for name-based lookup; BC-8.1.004 + BC-8.1.007).
        #[arg(long)]
        project: Option<String>,
        /// New component name (leading-dash values accepted, e.g. `--name -legacy`).
        #[arg(long, allow_hyphen_values = true)]
        name: Option<String>,
        /// New description (leading-dash values accepted).
        /// Pass an empty string (`--description ""`) to clear the description.
        #[arg(long, allow_hyphen_values = true)]
        description: Option<String>,
        /// New lead: account ID, display-name substring, email, or empty string
        /// to clear the lead (`--lead ""`; BC-8.1.007).
        #[arg(long)]
        lead: Option<String>,
    },
    /// Delete a component — requires an explicit disposition for its issues
    /// (BC-8.2.001 — BC-8.2.008, S-604-3).
    ///
    /// Irreversible: no trash/archive/undelete endpoint exists. Snapshots
    /// every affected issue key via a fully-paginated JQL search BEFORE the
    /// DELETE fires. Exactly one of `--move-to`/`--orphan` is required —
    /// neither supplied is an application-level exit-64 guard (NOT a clap
    /// `ArgGroup::required`, which would wrongly produce exit 2); both
    /// supplied is a clap `conflicts_with` exit 2.
    Delete {
        /// Component name (partial match) or numeric ID (BC-8.1.007/008 +
        /// BC-8.4.001 resolution semantics, reused here). Leading-dash names
        /// accepted (e.g. `-legacy`).
        #[arg(allow_hyphen_values = true)]
        name_or_id: String,
        /// Project key (required for name-based lookup; BC-8.1.004).
        #[arg(long)]
        project: Option<String>,
        /// Move this component's issues to another component (by name or
        /// numeric ID) before deleting it. Must resolve within the SAME
        /// project as the component being deleted (BC-8.2.002/003).
        /// Mutually exclusive with `--orphan`.
        #[arg(long, conflicts_with = "orphan")]
        move_to: Option<String>,
        /// Delete the component without moving its issues — they are left
        /// with no replacement component. Requires interactive confirmation
        /// (or `--yes` when non-interactive; BC-8.2.006). Mutually exclusive
        /// with `--move-to`.
        #[arg(long, conflicts_with = "move_to")]
        orphan: bool,
        /// Skip the `--orphan` interactive confirmation prompt (required when
        /// running non-interactively with `--orphan`; BC-8.2.006). No effect
        /// on `--move-to`, which never prompts (BC-8.2.006 Invariant 1).
        #[arg(long)]
        yes: bool,
    },
    /// Rename a component — single-project or `--all-projects` fan-out
    /// (BC-8.3.001 — BC-8.3.007, S-608-1).
    ///
    /// Single-project form (`--project KEY`): `--project` is UNCONDITIONALLY
    /// required (BC-8.3.001 Precondition 1) — no `.jr.toml` config fallback,
    /// no numeric-ID exemption (unlike `edit`/`delete`). `--all-projects`
    /// fans out across every accessible project containing a component named
    /// `OLD`, matched by EXACT case-insensitive equality — NOT the §8.4
    /// `partial_match` substring semantics used elsewhere in this command
    /// family (BC-8.3.002). Exactly one of `--project`/`--all-projects` is
    /// required — neither supplied is an application-level exit-64 guard
    /// (BC-8.3.005, DEC-188, mechanically identical to `delete`'s
    /// `--move-to`/`--orphan` split); both supplied is a clap
    /// `conflicts_with` exit 2. `--dry-run` is valid with either scope and
    /// performs the identical read-only discovery with zero mutating HTTP
    /// (BC-8.3.004).
    Rename {
        /// Current component name (partial match, single-project form only)
        /// or numeric ID. Leading-dash values accepted.
        #[arg(allow_hyphen_values = true)]
        old: String,
        /// New component name. Leading-dash values accepted.
        #[arg(allow_hyphen_values = true)]
        new: String,
        /// Project key — required for the single-project form (BC-8.3.001
        /// Precondition 1: unconditional, no `.jr.toml` fallback, no
        /// numeric-ID exemption). Mutually exclusive with `--all-projects`.
        #[arg(long, conflicts_with = "all_projects")]
        project: Option<String>,
        /// Fan out across every accessible project containing a component
        /// named `OLD`, matched by exact case-insensitive equality
        /// (BC-8.3.002). A numeric `OLD` is rejected pre-flight under this
        /// flag (BC-8.3.002 Precondition 2). Mutually exclusive with
        /// `--project`.
        #[arg(long, conflicts_with = "project")]
        all_projects: bool,
        /// Preview the rename set with zero mutating HTTP calls, using the
        /// identical discovery scope as the corresponding live run
        /// (BC-8.3.004).
        #[arg(long)]
        dry_run: bool,
    },
}

pub(crate) const DEFAULT_LIMIT: u32 = 30;

/// Resolve the effective limit from CLI flags.
///
/// Returns `None` when `--all` is set (no limit), otherwise returns the
/// explicit `--limit` value or the default.
pub(crate) fn resolve_effective_limit(limit: Option<u32>, all: bool) -> Option<u32> {
    if all {
        None
    } else {
        Some(limit.unwrap_or(DEFAULT_LIMIT))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_limit_defaults_to_30() {
        assert_eq!(resolve_effective_limit(None, false), Some(30));
    }

    #[test]
    fn effective_limit_respects_explicit_limit() {
        assert_eq!(resolve_effective_limit(Some(50), false), Some(50));
    }

    #[test]
    fn effective_limit_all_returns_none() {
        assert_eq!(resolve_effective_limit(None, true), None);
    }
}
