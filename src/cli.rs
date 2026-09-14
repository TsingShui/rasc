use crate::query::{ClassQuery, MemberQuery, Query, format_class_name, fuzzy_class_pattern};
use anyhow::{Result, bail};
use clap::{Args, Parser, Subcommand};
use std::path::{Path, PathBuf};

/// Default worker count for a command.
///
/// One worker per logical CPU. Every command runs the same shape of work over the
/// archive's DEX entries - inflate one entry, scan or index it - and the entry
/// scheduler has one entry in flight per worker, so the pool size is what decides
/// both how fast that parallel phase runs and how much memory it holds at once
/// (one inflated entry per worker at most). Measured on a 6P+6E machine with
/// `findrefs type View` over a 359 MiB archive: 1 / 8 / 12 threads scan in
/// 955 / 152 / 129 ms, and 16 threads is slower again (145 ms) because the extra
/// workers only contend. A hardcoded count cannot know any of that; the machine
/// can.
pub fn default_threads() -> usize {
    std::thread::available_parallelism().map_or(8, std::num::NonZero::get)
}

#[derive(Debug, Parser)]
#[command(name = "rasc", version, about = "Native Rust APK and DEX analysis CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

impl Command {
    /// Whether this invocation asked for records rather than the text payload.
    ///
    /// The failure envelope is decided by this, which is why it is a property of the
    /// command: the same error is a record or a sentence depending on what the caller said
    /// it wanted.
    pub fn is_json(&self) -> bool {
        match self {
            Command::Classes(args) => args.json,
            Command::Getclass(args) => args.json || args.outline,
            Command::Entries(args) => args.json,
            Command::Strings(args) => args.json,
            Command::Findrefs(args) => args.json,
            Command::Manifest(_) | Command::Skill(_) => false,
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Locate a class in an APK and decompile it.
    Getclass(GetClassArgs),
    /// List the entries an archive holds, in central-directory order.
    Entries(EntriesArgs),
    /// List every string in every root DEX of an archive.
    Strings(StringsArgs),
    /// Find code references across every DEX in an APK.
    Findrefs(FindRefsArgs),
    /// Decode and print AndroidManifest.xml from an APK.
    Manifest(ManifestArgs),
    /// List classes defined across every DEX in an APK.
    Classes(ClassesArgs),
    /// Install the rasc skill so coding agents know how to use rasc.
    Skill(SkillArgs),
}

#[derive(Debug, Args)]
pub struct SkillArgs {
    /// Install for these agents: pi, codex, claude, agents or all.
    ///
    /// Default: every agent that looks installed on this machine, or the cross-agent
    /// `~/.agents/skills` location when none of them is.
    #[arg(value_name = "AGENT")]
    pub agents: Vec<String>,
    /// Install into this skills directory instead of the agents' own ones.
    #[arg(short, long, value_name = "DIR")]
    pub dir: Option<PathBuf>,
    /// Print the skill to stdout instead of writing any file.
    #[arg(long)]
    pub print: bool,
}

#[derive(Debug, Args)]
pub struct ClassesArgs {
    #[arg(long, alias = "thread", default_value_t = default_threads())]
    pub threads: usize,
    /// Case-insensitive substring filter over Java-style class names.
    #[arg(short, long)]
    pub filter: Option<String>,
    /// One JSON object per row, `{"dex":…,"descriptor":…,"name":…}`, instead of the text
    /// columns. A failure is one `{"error":…}` record with the same message the text mode
    /// prints. Hosts read records; a row a name can break is a row a host can misread.
    #[arg(long)]
    pub json: bool,
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    /// Print phase timings on stderr.
    #[arg(long)]
    pub debug: bool,
    pub apk_path: PathBuf,
}

#[derive(Debug, Args)]
pub struct StringsArgs {
    /// One JSON object per string, `{"dex":…,"index":…,"value":…}`. String data holds
    /// whatever the file holds — newlines, quotes, any UTF-16 — and the text row cannot
    /// carry that without a host guessing where one ends.
    #[arg(long)]
    pub json: bool,
    /// Print only how many strings there are, as `{"count":N}`. The number is a DEX
    /// header field, so this reads no string data at all.
    #[arg(long, conflicts_with_all = ["filter", "limit"])]
    pub count: bool,
    /// Only strings containing this text, ignoring case. The same rule the workspace's
    /// own filter used, so a host that moves its filter here shows what it showed before.
    #[arg(long)]
    pub filter: Option<String>,
    /// At most this many strings, counted in the order they are printed.
    #[arg(long)]
    pub limit: Option<usize>,
    /// Start at this match instead of the first, counted in the order they are printed: with
    /// `--limit` this is the other half of a page, so a host that scrolls a searched table asks
    /// for what follows what it already has.
    #[arg(long, conflicts_with = "count")]
    pub offset: Option<usize>,
    /// How many methods use each string, as `{"dex":…,"index":…,"count":…}` records: one per
    /// *referenced* string, counted by distinct referencing methods, which is the rule
    /// `findrefs` reports a row per. A string nothing uses is absent rather than zero.
    #[arg(long, conflicts_with_all = ["count", "filter", "limit", "offset"])]
    pub xrefs: bool,
    #[arg(long, alias = "thread", default_value_t = default_threads())]
    pub threads: usize,
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    /// Print phase timings on stderr.
    #[arg(long)]
    pub debug: bool,
    pub apk_path: PathBuf,
}

#[derive(Debug, Args)]
pub struct EntriesArgs {
    /// One JSON object per entry, `{"name":…,"method":…,"compressed":…,"uncompressed":…,
    /// "offset":…}`, where `method` is the ZIP compression method (0 stored, 8 deflated)
    /// and `offset` is the entry's local header. A host that lists what an archive
    /// contains reads records; entry names are paths and hold anything.
    #[arg(long)]
    pub json: bool,
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    pub apk_path: PathBuf,
}

#[derive(Debug, Args)]
pub struct ManifestArgs {
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    pub apk_path: PathBuf,
}

#[derive(Debug, Args)]
pub struct GetClassArgs {
    /// One `{"members":[…]}` record before the source, describing the parts the document
    /// is made of: every declaration with its name and line range, plus the header and
    /// footer that no declaration owns. The source after the record is byte-identical to
    /// the source without this flag. A failure is one `{"error":…}` record.
    #[arg(long)]
    pub outline: bool,
    /// One `{"members":[…]}` record before the source, describing what the source contains.
    /// A failure is one `{"error":…}` record instead of the source.
    #[arg(long)]
    pub json: bool,
    #[arg(long)]
    pub debug: bool,
    #[arg(long, alias = "thread", default_value_t = default_threads())]
    pub threads: usize,
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    pub apk_path: PathBuf,
    pub dalvik_class: String,
}

#[derive(Debug, Args)]
pub struct FindRefsArgs {
    /// One JSON object per row, `{"dex":…,"member":…,"matched":…}`, instead of the text
    /// columns. A member name is a descriptor and a method name, and any of them can hold
    /// the separator, a quote or a newline; a host reads records for the same reason it
    /// does everywhere else. `matched` is the text mode's `matched=(…)` contents, without
    /// the wrapper. A failure is one `{"error":…}` record.
    #[arg(long)]
    pub json: bool,
    #[arg(long)]
    pub debug: bool,
    #[arg(long, alias = "thread", default_value_t = default_threads())]
    pub threads: usize,
    pub apk_path: PathBuf,
    #[command(subcommand)]
    pub kind: FindKind,
}

impl FindRefsArgs {
    pub fn query(&self) -> Result<Query> {
        match &self.kind {
            FindKind::String(q) => Ok(Query::String(q.value.clone())),
            FindKind::Type(q) => Ok(Query::Type(q.value.clone())),
            FindKind::Method(q) => Ok(Query::Method(q.member_query("method")?)),
            FindKind::Field(q) => Ok(Query::Field(q.member_query("field")?)),
        }
    }

    pub fn output_path(&self) -> Option<&Path> {
        match &self.kind {
            FindKind::String(q) | FindKind::Type(q) => q.output.as_deref(),
            FindKind::Method(q) | FindKind::Field(q) => q.output.as_deref(),
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum FindKind {
    String(ValueQuery),
    Type(ValueQuery),
    Method(MemberArgs),
    Field(MemberArgs),
}

#[derive(Debug, Args)]
pub struct ValueQuery {
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    pub value: String,
}

#[derive(Debug, Args)]
pub struct MemberArgs {
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    pub name: Option<String>,
    #[arg(long = "class")]
    pub class_name: Option<String>,
    #[arg(long)]
    pub fuzzy_class: bool,
}

impl MemberArgs {
    fn member_query(&self, kind: &str) -> Result<MemberQuery> {
        let name = self
            .name
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(str::to_owned);
        let class = match self.class_name.as_deref().filter(|name| !name.is_empty()) {
            None => None,
            Some(name) if self.fuzzy_class => Some(ClassQuery::Fuzzy(fuzzy_class_pattern(name))),
            Some(name) => Some(ClassQuery::Exact(format_class_name(name)?)),
        };
        if name.is_none() && class.is_none() {
            bail!("{kind} query needs at least one of class or {kind} name");
        }
        Ok(MemberQuery { name, class })
    }
}
