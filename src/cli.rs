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
    /// Print one class's DEX field layout as one JSON record.
    FieldsPlan(FieldsPlanArgs),
    /// Resolve a runtime field index to the DEX field behind it.
    FieldByIndex(FieldByIndexArgs),
    /// Resolve a runtime method index to the DEX method behind it.
    MethodByIndex(MethodByIndexArgs),
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
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    /// Print phase timings on stderr.
    #[arg(long)]
    pub debug: bool,
    pub apk_path: PathBuf,
}

#[derive(Debug, Args)]
pub struct FieldsPlanArgs {
    /// One or more code inputs - an APK, or bare DEX files - tried in this order.
    #[arg(long = "apk", required = true, num_args = 1.., value_name = "FILE")]
    pub apk: Vec<PathBuf>,
    /// Class to plan: `Lcom/foo/Bar;` or `com.foo.Bar`.
    #[arg(long)]
    pub descriptor: String,
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    #[arg(long, alias = "thread", default_value_t = default_threads())]
    pub threads: usize,
    /// Print phase timings on stderr.
    #[arg(long)]
    pub debug: bool,
}

#[derive(Debug, Args)]
pub struct FieldByIndexArgs {
    /// One or more code inputs - an APK, or bare DEX files - tried in this order.
    #[arg(long = "apk", required = true, num_args = 1.., value_name = "FILE")]
    pub apk: Vec<PathBuf>,
    /// Declaring class: `Lcom/foo/Bar;` or `com.foo.Bar`.
    #[arg(long)]
    pub descriptor: String,
    /// The index a runtime reports for a field: instance fields first, then statics
    /// (`ifields_` then `sfields_` order). Not the `field_ids` index - the row prints
    /// that one too, so the two can be told apart.
    #[arg(long)]
    pub field_index: u32,
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    #[arg(long, alias = "thread", default_value_t = default_threads())]
    pub threads: usize,
    /// Print phase timings on stderr.
    #[arg(long)]
    pub debug: bool,
}

#[derive(Debug, Args)]
pub struct MethodByIndexArgs {
    /// One or more code inputs - an APK, or bare DEX files - tried in this order.
    #[arg(long = "apk", required = true, num_args = 1.., value_name = "FILE")]
    pub apk: Vec<PathBuf>,
    /// Declaring class: `Lcom/foo/Bar;` or `com.foo.Bar`.
    #[arg(long)]
    pub descriptor: String,
    /// The index a runtime reports for a method: the `method_ids` index.
    #[arg(long)]
    pub method_index: u32,
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    #[arg(long, alias = "thread", default_value_t = default_threads())]
    pub threads: usize,
    /// Print phase timings on stderr.
    #[arg(long)]
    pub debug: bool,
}

#[derive(Debug, Args)]
pub struct StringsArgs {
    /// Only strings containing this text, ignoring case.
    #[arg(long)]
    pub filter: Option<String>,
    /// At most this many strings, counted in the order they are printed.
    #[arg(long)]
    pub limit: Option<usize>,
    /// Start at this match instead of the first, counted in the order they are printed: with
    /// `--limit` this is the other half of a page.
    #[arg(long)]
    pub offset: Option<usize>,
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
