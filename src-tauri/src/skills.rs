use crate::deepseek::DeepSeekClient;
use crate::error::{AppError, AppResult};
use include_dir::{include_dir, Dir};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::{Path, PathBuf};

/// Bundled skills embedded at compile time from src-tauri/skills/.
pub static BUNDLED_SKILLS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/skills");

/// Seed bundled skills into the app data dir (never overwrites user edits).
pub fn seed_skills(data_dir: &Path) {
    let target = data_dir.join("skills");
    for file in walk_files(&BUNDLED_SKILLS) {
        let rel = file
            .path()
            .strip_prefix(BUNDLED_SKILLS.path())
            .unwrap_or(file.path());
        let dest = target.join(rel);
        if dest.exists() {
            continue;
        }
        if let Some(parent) = dest.parent() {
            if std::fs::create_dir_all(parent).is_err() {
                continue;
            }
        }
        let _ = std::fs::write(dest, file.contents());
    }
}

fn walk_files<'a>(dir: &'a Dir<'a>) -> Vec<&'a include_dir::File<'a>> {
    let mut out = Vec::new();
    collect_files(dir, &mut out);
    out
}

fn collect_files<'a>(dir: &'a Dir<'a>, out: &mut Vec<&'a include_dir::File<'a>>) {
    for entry in dir.entries() {
        match entry {
            include_dir::DirEntry::Dir(d) => collect_files(d, out),
            include_dir::DirEntry::File(f) => out.push(f),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SkillInfo {
    pub name: String,
    pub path: PathBuf,
    pub description: String,
    pub implicit: bool,
    pub enabled: bool,
}

#[derive(Debug, Deserialize, Default)]
struct SkillFrontmatter {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    metadata: FrontmatterMetadata,
}

#[derive(Debug, Deserialize, Default)]
struct FrontmatterMetadata {
    #[serde(default, rename = "allow-implicit-invocation")]
    allow_implicit_invocation: Option<bool>,
}

/// Scan skill roots for `SKILL.md` definitions, deepest priority first.
/// `enabled_override` is the settings.enabled_skills list; empty = all enabled.
pub fn scan_skills(roots: &[PathBuf], enabled_override: &[String]) -> Vec<SkillInfo> {
    let mut out: Vec<SkillInfo> = Vec::new();
    for root in roots {
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            let skill_dir = entry.path();
            if !skill_dir.is_dir() {
                continue;
            }
            let skill_md = skill_dir.join("SKILL.md");
            if !skill_md.exists() {
                continue;
            }
            let Some((name, description, implicit)) = parse_frontmatter(&skill_md) else {
                continue;
            };
            let enabled = enabled_override.is_empty() || enabled_override.contains(&name);
            // dedupe by name (first wins = highest priority root)
            if out.iter().any(|s| s.name == name) {
                continue;
            }
            out.push(SkillInfo {
                name,
                path: skill_dir,
                description,
                implicit,
                enabled,
            });
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn parse_frontmatter(path: &Path) -> Option<(String, String, bool)> {
    let raw = std::fs::read_to_string(path).ok()?;
    let trimmed = raw.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }
    let rest = &trimmed[3..];
    let end = rest.find("---")?;
    let yaml = &rest[..end];
    let body = rest[end + 3..].trim().to_string();
    let fm: SkillFrontmatter = serde_yaml::from_str(yaml).ok()?;
    let name = fm
        .name
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| {
            path.parent()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().replace('_', "-"))
                .unwrap_or_else(|| "unnamed".into())
        });
    let description = fm.description.unwrap_or_default();
    let implicit = fm
        .metadata
        .allow_implicit_invocation
        .unwrap_or(true);
    // fall back to body head when description is missing
    let description = if description.trim().is_empty() {
        body.lines()
            .find(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
            .map(|l| l.trim().to_string())
            .unwrap_or_default()
    } else {
        description
    };
    Some((name, description, implicit))
}

/// Let the model pick the most relevant skills for the user message.
/// Mirrors deepcode-cli's JSON-object skill selection.
pub async fn select_skills(
    client: &DeepSeekClient,
    skills: &[SkillInfo],
    user_message: &str,
    model: &str,
) -> AppResult<Vec<SkillInfo>> {
    if skills.is_empty() {
        return Ok(Vec::new());
    }
    let catalog: Vec<String> = skills
        .iter()
        .map(|s| format!("- {}: {}", s.name, s.description))
        .collect();
    let prompt = format!(
        "Below are the available skills. Choose at most 3 that are most relevant to help with the user's request. Return ONLY a JSON object like {{\"skillNames\": [\"skill-a\", \"skill-b\"]}} (empty array if none apply).\n\nAvailable skills:\n{}\n\nUser request: {}",
        catalog.join("\n"),
        user_message
    );
    let body = json!({
        "model": model,
        "messages": [
            { "role": "system", "content": "You are a skill router. Select relevant skills by name. Reply with JSON only." },
            { "role": "user", "content": prompt }
        ],
        "temperature": 0.1,
        "response_format": { "type": "json_object" }
    });
    let resp = client.chat_json(body).await?;
    let content = resp["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| AppError::Parse("技能选择响应缺少 content".into()))?;
    let parsed: serde_json::Value = serde_json::from_str(content).map_err(|_| {
        AppError::Parse(format!("技能选择响应不是合法 JSON: {}", content.chars().take(200).collect::<String>()))
    })?;
    let mut picked: Vec<String> = Vec::new();
    if let Some(names) = parsed["skillNames"].as_array() {
        for n in names {
            if let Some(n) = n.as_str() {
                if !picked.contains(&n.to_string()) {
                    picked.push(n.to_string());
                }
            }
        }
    }
    let selected: Vec<SkillInfo> = skills
        .iter()
        .filter(|s| picked.contains(&s.name))
        .cloned()
        .collect();
    Ok(selected)
}

/// Render matched skill documents as a system-prompt injection block.
pub fn build_skill_documents(skills: &[SkillInfo]) -> String {
    let mut out = String::from("Use the skill documents below to assist the user:\n");
    for s in skills {
        let body = std::fs::read_to_string(s.path.join("SKILL.md")).unwrap_or_default();
        out.push_str(&format!(
            "<{name} path=\"{path}\">\n{body}\n</{name}>\n",
            name = s.name,
            path = s.path.display()
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_skill(dir: &Path, name: &str, description: &str, implicit: bool) {
        let skill_dir = dir.join(name);
        std::fs::create_dir_all(&skill_dir).unwrap();
        let fm = format!(
            "---\nname: {name}\ndescription: {description}\nmetadata:\n  allow-implicit-invocation: {implicit}\n---\n\n# {name}\n\nDo the thing.\n"
        );
        std::fs::write(skill_dir.join("SKILL.md"), fm).unwrap();
    }

    #[test]
    fn scans_and_parses_skills() {
        let dir = tempfile::tempdir().unwrap();
        write_skill(dir.path(), "code-review", "Review code for bugs", true);
        write_skill(dir.path(), "doc-writer", "Write documentation", false);

        let skills = scan_skills(&[dir.path().to_path_buf()], &[]);
        assert_eq!(skills.len(), 2);
        let review = skills.iter().find(|s| s.name == "code-review").unwrap();
        assert!(review.implicit);
        assert!(review.description.contains("Review code"));
        assert!(review.enabled);
        let doc = skills.iter().find(|s| s.name == "doc-writer").unwrap();
        assert!(!doc.implicit);
    }

    #[test]
    fn enabled_override_filters() {
        let dir = tempfile::tempdir().unwrap();
        write_skill(dir.path(), "a", "skill a", true);
        write_skill(dir.path(), "b", "skill b", true);
        let skills = scan_skills(&[dir.path().to_path_buf()], &["a".to_string()]);
        assert_eq!(skills.len(), 2);
        assert!(skills.iter().find(|s| s.name == "a").unwrap().enabled);
        assert!(!skills.iter().find(|s| s.name == "b").unwrap().enabled);
    }

    #[test]
    fn falls_back_to_dir_name() {
        let dir = tempfile::tempdir().unwrap();
        let skill_dir = dir.path().join("my_skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\ndescription: something\n---\n\nbody",
        )
        .unwrap();
        let skills = scan_skills(&[dir.path().to_path_buf()], &[]);
        assert_eq!(skills[0].name, "my-skill");
        assert_eq!(skills[0].description, "something");
    }
}
