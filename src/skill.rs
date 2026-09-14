//! `rasc skill`: installs the bundled rasc skill for coding agents.
//!
//! The skill is a single `SKILL.md` in the Agent Skills layout
//! (`<skills-dir>/<name>/SKILL.md`), embedded in the binary so a release build can set up an
//! agent without a checkout. Installing is a plain file write rather than a call into the
//! agents' own tooling, and each target is the user-level directory that agent documents:
//!
//! | Agent | User-level skills directory |
//! |---|---|
//! | pi | `~/.pi/agent/skills` |
//! | Codex | `$CODEX_HOME/skills`, else `~/.codex/skills` |
//! | Claude Code | `~/.claude/skills` |
//! | cross-agent | `~/.agents/skills` |
//!
//! Only the native and WASI hosts can write files. The JS host build has no filesystem, so
//! it is left with `--print`, which turns the skill into the command's stdout payload.

use anyhow::{Context, Result, bail};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// The frontmatter `name` and directory name of the installed skill.
pub const SKILL_NAME: &str = "rasc";

/// The skill text, written verbatim.
///
/// Kept as a plain file under `skill/` so it is edited and reviewed as documentation, not
/// escaped inside a Rust string. `--print` emits exactly these bytes, which is what makes
/// that flag useful for agents this command does not know about.
pub const SKILL_MD: &str = include_str!("../skill/SKILL.md");

/// An agent whose skill directory `rasc skill` knows how to write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Agent {
    Pi,
    Codex,
    Claude,
    /// The cross-agent `.agents/skills/` convention from the Agent Skills specification.
    Agents,
}

impl Agent {
    /// Every agent, in the order `all` installs them.
    pub const ALL: [Agent; 4] = [Agent::Pi, Agent::Codex, Agent::Claude, Agent::Agents];

    /// Parses the `AGENT` arguments. `all` expands, duplicates collapse.
    fn parse(names: &[String]) -> Result<Vec<Agent>> {
        let mut selected: Vec<Agent> = Vec::new();
        for name in names {
            let expanded: &[Agent] = match name.to_ascii_lowercase().as_str() {
                "pi" => &[Agent::Pi],
                "codex" => &[Agent::Codex],
                "claude" | "claude-code" => &[Agent::Claude],
                "agents" => &[Agent::Agents],
                "all" => &Agent::ALL,
                other => bail!("unknown agent `{other}` (want pi, codex, claude, agents or all)"),
            };
            for agent in expanded {
                if !selected.contains(agent) {
                    selected.push(*agent);
                }
            }
        }
        Ok(selected)
    }

    /// The user-level skills directory this agent scans.
    ///
    /// `codex_home` is passed in instead of read from the environment here, so the paths
    /// stay testable without touching process-wide state.
    fn skills_dir(self, home: &Path, codex_home: &Path) -> PathBuf {
        match self {
            Agent::Pi => home.join(".pi/agent/skills"),
            Agent::Codex => codex_home.join("skills"),
            Agent::Claude => home.join(".claude/skills"),
            Agent::Agents => home.join(".agents/skills"),
        }
    }

    /// Whether this agent looks like it is in use on this machine.
    ///
    /// Only each agent's own directory is consulted, so a machine using none of them ends up
    /// with the cross-agent location instead of a fresh dotdir per unused tool.
    fn detected(self, home: &Path, codex_home: &Path) -> bool {
        match self {
            Agent::Pi => home.join(".pi").is_dir(),
            Agent::Codex => codex_home.is_dir(),
            Agent::Claude => home.join(".claude").is_dir(),
            Agent::Agents => home.join(".agents/skills").is_dir(),
        }
    }
}

/// `rasc skill` without `AGENT` installs for every agent that looks installed; when none
/// does, the cross-agent location is the only sensible place left.
fn default_agents(home: &Path, codex_home: &Path) -> Vec<Agent> {
    let installed: Vec<Agent> = [Agent::Pi, Agent::Codex, Agent::Claude]
        .into_iter()
        .filter(|agent| agent.detected(home, codex_home))
        .collect();
    if installed.is_empty() {
        vec![Agent::Agents]
    } else {
        installed
    }
}

/// The `SKILL.md` path inside a skills directory.
fn skill_path(skills_dir: &Path) -> PathBuf {
    skills_dir.join(SKILL_NAME).join("SKILL.md")
}

/// Resolves the `SKILL.md` paths to write from the command line.
///
/// An empty `agents` means auto-detection. `--dir`, when given, is the whole destination - a
/// skills directory such as `.claude/skills`, not the skill directory itself - and cannot be
/// combined with an explicit `AGENT`, because silently ignoring one of the two would hide a
/// typo in the other.
pub fn targets(agents: &[String], dir: Option<&Path>) -> Result<Vec<PathBuf>> {
    if let Some(dir) = dir {
        if !agents.is_empty() {
            bail!("AGENT and --dir are mutually exclusive; --dir already names one destination");
        }
        return Ok(vec![skill_path(dir)]);
    }
    let home = home_dir()?;
    let codex_home = codex_home(&home);
    let agents = if agents.is_empty() {
        default_agents(&home, &codex_home)
    } else {
        Agent::parse(agents)?
    };
    Ok(agents
        .into_iter()
        .map(|agent| skill_path(&agent.skills_dir(&home, &codex_home)))
        .collect())
}

/// What installing at one path did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The skill was not there before.
    Installed,
    /// An existing skill was replaced.
    Updated,
    /// The file already had exactly this content.
    Unchanged,
}

impl Outcome {
    /// The word the CLI prints in front of the path.
    pub fn verb(self) -> &'static str {
        match self {
            Outcome::Installed => "installed",
            Outcome::Updated => "updated",
            Outcome::Unchanged => "unchanged",
        }
    }
}

/// Writes the bundled skill to `path`, creating the skill directory.
///
/// Re-running this command is the normal way to update the skill, so an identical file is
/// left untouched and reported as such instead of being rewritten.
pub fn install(path: &Path) -> Result<Outcome> {
    let outcome = match fs::read_to_string(path) {
        Ok(current) if current == SKILL_MD => return Ok(Outcome::Unchanged),
        Ok(_) => Outcome::Updated,
        // Not readable means "not installed here", which is the same as absent; a path that
        // cannot be written surfaces below rather than being guessed at.
        Err(_) => Outcome::Installed,
    };
    let directory = path.parent().context("skill path has no directory")?;
    fs::create_dir_all(directory).with_context(|| format!("create {}", directory.display()))?;
    fs::write(path, SKILL_MD).with_context(|| format!("write {}", path.display()))?;
    Ok(outcome)
}

/// The home directory, as the agents themselves resolve it.
fn home_dir() -> Result<PathBuf> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .context("cannot determine the home directory (HOME is unset); use --dir")
}

/// Codex reads `$CODEX_HOME`; `~/.codex` is only the fallback.
fn codex_home(home: &Path) -> PathBuf {
    env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| home.join(".codex"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A unique empty directory. The caller removes it with `fs::remove_dir_all`.
    fn temp_dir() -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = env::temp_dir().join(format!(
            "rasc-skill-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    /// The value of a `key: value` frontmatter line.
    fn frontmatter(key: &str) -> String {
        let prefix = format!("{key}: ");
        SKILL_MD
            .lines()
            .skip(1)
            .take_while(|line| *line != "---")
            .find_map(|line| line.strip_prefix(&prefix))
            .unwrap_or_else(|| panic!("frontmatter has no `{key}`"))
            .trim()
            .to_owned()
    }

    #[test]
    fn the_bundled_skill_has_valid_frontmatter() {
        assert!(
            SKILL_MD.starts_with("---\n"),
            "the file must open with YAML frontmatter"
        );
        let name = frontmatter("name");
        assert_eq!(name, SKILL_NAME);
        assert!(
            name.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
            "skill names are lowercase a-z, 0-9 and hyphens"
        );
        let description = frontmatter("description");
        assert!(!description.is_empty() && description.chars().count() <= 1024);
        // The whole point of the file is telling an agent which commands exist.
        for command in ["getclass", "findrefs", "classes", "manifest"] {
            assert!(
                SKILL_MD.contains(command),
                "the skill no longer mentions `{command}`"
            );
        }
    }

    #[test]
    fn each_agent_uses_its_documented_directory() {
        let home = Path::new("/home/user");
        let codex = Path::new("/opt/codex-home");
        assert_eq!(
            Agent::Pi.skills_dir(home, codex),
            Path::new("/home/user/.pi/agent/skills")
        );
        assert_eq!(
            Agent::Codex.skills_dir(home, codex),
            Path::new("/opt/codex-home/skills")
        );
        assert_eq!(
            Agent::Claude.skills_dir(home, codex),
            Path::new("/home/user/.claude/skills")
        );
        assert_eq!(
            Agent::Agents.skills_dir(home, codex),
            Path::new("/home/user/.agents/skills")
        );
    }

    #[test]
    fn agent_arguments_parse_expand_and_deduplicate() {
        // A closure here would be reflowed into one line by rustfmt; a local fn stays readable.
        fn names(list: &[&str]) -> Vec<String> {
            list.iter().map(|name| (*name).to_owned()).collect()
        }
        assert_eq!(Agent::parse(&names(&["all"])).unwrap(), Agent::ALL);
        assert_eq!(
            Agent::parse(&names(&["PI", "pi", "claude-code"])).unwrap(),
            vec![Agent::Pi, Agent::Claude]
        );
        assert!(Agent::parse(&names(&["vscode"])).is_err());
    }

    #[test]
    fn auto_detection_picks_installed_agents_and_falls_back() {
        let home = temp_dir();
        let codex = home.join("codex-home");
        assert_eq!(default_agents(&home, &codex), vec![Agent::Agents]);

        fs::create_dir_all(home.join(".pi")).unwrap();
        assert_eq!(default_agents(&home, &codex), vec![Agent::Pi]);

        fs::create_dir_all(home.join(".claude")).unwrap();
        fs::create_dir_all(&codex).unwrap();
        assert_eq!(
            default_agents(&home, &codex),
            vec![Agent::Pi, Agent::Codex, Agent::Claude]
        );
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn install_writes_the_skill_and_is_idempotent() {
        let root = temp_dir();
        let path = skill_path(&root.join("skills"));
        assert!(!path.exists());

        assert_eq!(install(&path).unwrap(), Outcome::Installed);
        assert_eq!(fs::read_to_string(&path).unwrap(), SKILL_MD);
        assert_eq!(install(&path).unwrap(), Outcome::Unchanged);

        fs::write(&path, "stale").unwrap();
        assert_eq!(install(&path).unwrap(), Outcome::Updated);
        assert_eq!(fs::read_to_string(&path).unwrap(), SKILL_MD);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn dir_is_one_destination_and_excludes_agents() {
        let dir = Path::new("/tmp/project/.claude/skills");
        assert_eq!(
            targets(&[], Some(dir)).unwrap(),
            vec![Path::new("/tmp/project/.claude/skills/rasc/SKILL.md")]
        );
        assert!(targets(&["pi".to_owned()], Some(dir)).is_err());
    }
}
