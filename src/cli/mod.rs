pub mod assets;
pub mod auth;
pub mod board;
pub mod init;
pub mod issue;
pub mod project;
pub mod queue;
pub mod sprint;
pub mod team;
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

    /// Enable verbose output
    #[arg(long, global = true)]
    pub verbose: bool,
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
        command: IssueCommand,
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
    /// Manage JSM queues
    Queue {
        #[command(subcommand)]
        command: QueueCommand,
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
        /// Include object attributes in output
        #[arg(long)]
        attributes: bool,
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
}

#[derive(Subcommand)]
pub enum AuthCommand {
    /// Authenticate with Jira
    Login {
        /// Use OAuth 2.0 instead of API token (requires your own OAuth app)
        #[arg(long)]
        oauth: bool,
    },
    /// Show authentication status
    Status,
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
        /// Show only open issues (excludes Done status category)
        #[arg(long, conflicts_with = "status")]
        open: bool,
        /// Show story points column
        #[arg(long)]
        points: bool,
        /// Show linked assets column
        #[arg(long)]
        assets: bool,
        /// Filter by linked asset object key (e.g., CUST-5)
        #[arg(long)]
        asset: Option<String>,
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
        #[arg(short, long)]
        summary: Option<String>,
        /// Description
        #[arg(short, long, conflicts_with = "description_stdin")]
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
    },
    /// View issue details
    View {
        /// Issue key (e.g., FOO-123)
        key: String,
    },
    /// Edit issue fields
    Edit {
        /// Issue key
        key: String,
        /// New summary
        #[arg(long)]
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
        #[arg(long)]
        parent: Option<String>,
        /// Description
        #[arg(short, long, conflicts_with = "description_stdin")]
        description: Option<String>,
        /// Read description from stdin (for piping)
        #[arg(long, conflicts_with = "description")]
        description_stdin: bool,
        /// Interpret description as Markdown
        #[arg(long)]
        markdown: bool,
    },
    /// Transition issue to a new status
    Move {
        /// Issue key
        key: String,
        /// Target status (partial match supported)
        status: Option<String>,
    },
    /// List available transitions without performing one
    Transitions {
        /// Issue key
        key: String,
    },
    /// Assign issue
    Assign {
        /// Issue key
        key: String,
        /// Assign to this user (omit to assign to self)
        #[arg(long)]
        to: Option<String>,
        /// Remove assignee
        #[arg(long)]
        unassign: bool,
    },
    /// Add a comment
    Comment {
        /// Issue key
        key: String,
        /// Comment text
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
    },
    /// List comments on an issue
    Comments {
        /// Issue key (e.g., FOO-123)
        key: String,
        /// Maximum number of comments to return
        #[arg(long)]
        limit: Option<u32>,
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
    Unlink {
        /// First issue key
        key1: String,
        /// Second issue key
        key2: String,
        /// Only remove links of this type (removes all if omitted)
        #[arg(long)]
        r#type: Option<String>,
    },
    /// List available link types
    LinkTypes,
    /// Show assets linked to an issue
    Assets {
        /// Issue key (e.g., FOO-123)
        key: String,
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
pub enum WorklogCommand {
    /// Log time on an issue
    Add {
        /// Issue key
        key: String,
        /// Duration (e.g., 2h, 1h30m, 1d)
        duration: String,
        /// Comment
        #[arg(short, long)]
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
