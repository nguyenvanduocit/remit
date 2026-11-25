//! The tools that fill the library. The owner never edits it himself: he tells an agent what he wants, and the Chair of a
//! chain that holds the `library` capability (an author chain's) turns it into these calls. A credential is the one
//! thing it is never given: the owner types it into the inbox (credentials.rs), so no tool of the library takes one.

use std::sync::Arc;

use std::collections::BTreeMap;

use model::Skill;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Capabilities;
use crate::agent_tools::{Author, tools_label};
use chains::tools::NoInput;
use chains::tools::{Effect, Tools, as_json, is_zero_rev};
use chains::{Error, Result};

/// One file of a skill; the files go as a list because a map is awkward for a model to write.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SkillFileInput {
    /// path inside the skill's directory, with slashes, e.g. SKILL.md or references/api.md
    pub path: String,
    /// the file's text
    pub content: String,
}

/// A skill in the shape the tools take, so what read_skill returns can go straight back into update_skill.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SkillInput {
    /// lower-case words joined by single dashes or dots, e.g. frontend-tokens; it never changes after creation, and
    /// SKILL.md must name the skill the same
    pub name: String,
    /// what the skill is for and when to use it: it is what the model reads to decide whether to load the skill; the same
    /// as the description in SKILL.md
    pub description: String,
    /// the kind of work it is for, in lowercase words like engineering or data: its column on the Skills page. Reuse a
    /// group list_library already shows; empty is general
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub group: String,
    /// every file of the skill. SKILL.md is required and starts with a --- block giving name and description, then the
    /// instructions; other text files (references, scripts) are optional
    pub files: Vec<SkillFileInput>,
    /// the revision read_skill returned: update_skill writes over that revision only, so a change made since is never lost.
    /// Leave it out to create
    #[serde(default, skip_serializing_if = "is_zero_rev")]
    pub rev: i64,
}

/// A skill, named.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SkillNameInput {
    /// skill name, as list_library shows it
    name: String,
}

/// What edit_skill_file takes: one change to one file, by the text it replaces.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct EditSkillFileInput {
    /// skill name, as list_library shows it
    name: String,
    /// the file's path inside the skill, as read_skill shows it, e.g. SKILL.md
    path: String,
    /// the exact text to change, copied from the file as read_skill shows it, with enough of the lines around it that
    /// the file holds it once
    old: String,
    /// what replaces it
    new: String,
}

/// What an import takes.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ImportInput {
    /// the name list_importable shows
    name: String,
    /// the kind of work it is for, in lowercase words like engineering or data: its column on the page that lists it.
    /// Reuse a group the library already shows; empty is general
    #[serde(default)]
    group: String,
    /// true to overwrite one the library already has; without it that is refused
    #[serde(default)]
    replace: bool,
}

impl SkillInput {
    fn skill(self) -> Result<Skill> {
        let mut files = BTreeMap::new();
        for f in self.files {
            if files.contains_key(&f.path) {
                return Err(Error::refused(format!(
                    "file {:?} is given twice: give each path once",
                    f.path
                )));
            }
            files.insert(f.path, f.content);
        }
        Ok(Skill {
            name: self.name,
            description: self.description,
            group: self.group,
            files,
            rev: self.rev,
            ..Skill::default()
        })
    }
}

fn skill_input(s: Skill) -> SkillInput {
    SkillInput {
        name: s.name,
        description: s.description,
        group: s.group,
        files: s
            .files
            .into_iter()
            .map(|(path, content)| SkillFileInput { path, content })
            .collect(),
        rev: s.rev,
    }
}

/// A skill as list_library says it.
#[derive(Serialize)]
struct SkillRow {
    name: String,
    description: String,
    group: String,
    source: String,
    files: Vec<String>,
}

/// The library as list_library says it.
#[derive(Serialize)]
struct LibraryRows {
    skills: Vec<SkillRow>,
}

fn library_listing(skills: Vec<Skill>) -> LibraryRows {
    LibraryRows {
        skills: skills
            .into_iter()
            .map(|s| SkillRow {
                source: source_text(s.source.as_ref()),
                files: s.files.into_keys().collect(),
                name: s.name,
                description: s.description,
                group: s.group,
            })
            .collect(),
    }
}

fn source_text(s: Option<&model::Source>) -> String {
    match s {
        None => "written here".to_owned(),
        Some(s) => format!("{}: {}", s.kind, s.name),
    }
}

/// What an import left out, one line each; empty when it took everything.
fn report_text(r: &importer::Report) -> String {
    let mut b = String::new();
    for s in &r.skipped {
        b.push_str(&format!("\nLeft out {}: {}.", s.what, s.why));
    }
    if !r.unmapped.is_empty() {
        b.push_str(&format!(
            "\nNot kept, the library has no place for it: {}.",
            r.unmapped.join(", ")
        ));
    }
    b
}

fn importable_text(found: &[importer::Candidate]) -> String {
    if found.is_empty() {
        return "The Claude Code configuration on this machine holds no agents or skills.".to_owned();
    }
    let mut b = String::new();
    for c in found {
        if !c.problem.is_empty() {
            b.push_str(&format!("{} {}: cannot be imported: {}\n", c.kind, c.name, c.problem));
            continue;
        }
        b.push_str(&format!(
            "{} {}: {} ({} bytes)\n",
            c.kind, c.name, c.description, c.size
        ));
    }
    b
}

impl Capabilities {
    /// The tools that read the library: every Chair's, to hire from it, an author chain's and the Assistant's.
    pub fn add_library_read_tools(self: &Arc<Self>, t: &mut Tools) {
        t.serving(self).add(
            "list_library", Effect::Reads,
            "The skills agents can hold: for each its name, description, group (its column on the Skills page), where it came from (or written here) and its files. The tools agents read the outside world through are Remit's own sources: list_sources shows them.",
            |o, _: NoInput| async move {
                let held = o.library_of().await?;
                as_json(&library_listing(held.skills))
            },
        );
        t.serving(self).add(
            "read_skill",
            Effect::Reads,
            "One skill in full, in the shape create_skill and update_skill take.",
            |o, i: SkillNameInput| async move {
                let key = i.name.clone();
                let s = o
                    .store
                    .read(move |r| r.skill(&key))
                    .await
                    .map_err(|e| Error::refused(format!("skill {:?}: {e}", i.name)))?;
                as_json(&skill_input(s))
            },
        );
    }

    /// The tool that reads what the machine's Claude Code configuration could give the library: an author chain's,
    /// and the Assistant's. A Chair that only hires has no use for it.
    pub fn add_importable_tool(self: &Arc<Self>, t: &mut Tools) {
        t.serving(self).add(
            "list_importable", Effect::Reads,
            "What the Claude Code configuration on this machine holds that the library can import: agents and skills, each with its description and size, or why it cannot be imported. Read only; it never shows a credential.",
            |o, _: NoInput| async move { Ok(importable_text(&o.importable().await?)) },
        );
    }

    /// The tools that change the library; au records each change where the owner sees it.
    pub fn add_library_write_tools(self: &Arc<Self>, t: &mut Tools, au: &Author) {
        self.add_skill_write_tools(t, au);
        self.add_import_tools(t, au);
    }

    /// create_skill, update_skill, edit_skill_file and delete_skill.
    fn add_skill_write_tools(self: &Arc<Self>, t: &mut Tools, au: &Author) {
        let a = au.clone();
        t.serving(self).add(
            "create_skill", Effect::Writes,
            "Write a new skill: instructions an agent that holds it can load when its work calls for them. A name already in use is refused. An agent holds it once an agent definition lists its name in skills.",
            move |o, i: SkillInput| {
                let au = a.clone();
                async move {
                    let by = au.by(&o).await?;
                    let written = match i.skill() {
                        Ok(s) => o.create_skill(&by, s).await,
                        Err(err) => Err(err),
                    };
                    au.answer(&o, written, |s| {
                        (
                            format!("Create skill {}.", s.name),
                            format!("Created skill {} ({} files).", s.name, s.files.len()),
                        )
                    })
                    .await
                }
            },
        );
        let a = au.clone();
        t.serving(self).add(
            "update_skill", Effect::Writes,
            "Replace an existing skill with this version, made from what read_skill returned (its rev included): every file is replaced, so keep what the owner did not ask to change. To change some lines of one file, edit_skill_file does it without sending the rest. The name stays. A skill changed since that read is not written over: the answer says so, and you read it again. Agents hold it by name, so every session that starts after this one loads the new version.",
            move |o, i: SkillInput| {
                let au = a.clone();
                async move {
                    let by = au.by(&o).await?;
                    let written = match i.skill() {
                        Ok(s) => o.update_skill(&by, s).await,
                        Err(err) => Err(err),
                    };
                    au.answer(&o, written, |s| {
                        (
                            format!("Update skill {}.", s.name),
                            format!("Updated skill {} ({} files).", s.name, s.files.len()),
                        )
                    })
                    .await
                }
            },
        );
        let a = au.clone();
        t.serving(self).add(
            "edit_skill_file", Effect::Writes,
            "Change some lines of one file of a skill, without sending the rest: old is the exact text to change, quoted from read_skill with enough of the lines around it that the file holds it once, and new replaces it. Every other file and line stays as it is stored. Text the file no longer holds is refused: it changed since you read it, so read_skill again.",
            move |o, i: EditSkillFileInput| {
                let au = a.clone();
                async move {
                    let by = au.by(&o).await?;
                    let written = o.edit_skill_file(&by, &i.name, &i.path, &i.old, &i.new).await;
                    au.answer(&o, written, |s| {
                        (
                            format!("Edit {} of skill {}.", i.path, s.name),
                            format!("Edited {} of skill {}.", i.path, s.name),
                        )
                    })
                    .await
                }
            },
        );
        let a = au.clone();
        t.serving(self).add(
            "delete_skill",
            Effect::Writes,
            "Delete a skill the owner named. A skill an agent holds cannot be deleted: change that agent first.",
            move |o, i: SkillNameInput| {
                let au = a.clone();
                async move {
                    let by = au.by(&o).await?;
                    o.delete_skill(&by, &i.name).await?;
                    au.record(&o, &format!("Delete skill {}.", i.name)).await?;
                    Ok(format!("Deleted skill {}.", i.name))
                }
            },
        );
    }

    /// import_skill and import_agent_definition: they bring into the library what the machine's Claude Code
    /// configuration holds (list_importable lists it).
    fn add_import_tools(self: &Arc<Self>, t: &mut Tools, au: &Author) {
        let a = au.clone();
        t.serving(self).add(
            "import_skill", Effect::Writes,
            "Bring in a skill from the Claude Code configuration on this machine (list_importable shows them). Its text files are copied; the result lists every file left out and why. A name the library has is refused unless replace is true.",
            move |o, i: ImportInput| {
                let au = a.clone();
                async move {
                    let by = au.by(&o).await?;
                    let written = o.import_skill(&by, &i.name, &i.group, i.replace).await;
                    au.answer(&o, written, |(s, report)| {
                        (
                            format!("Import skill {} from Claude Code.", s.name),
                            format!("Imported skill {} ({} files).{}", s.name, s.files.len(), report_text(&report)),
                        )
                    })
                    .await
                }
            },
        );
        let a = au.clone();
        t.serving(self).add(
            "import_agent_definition", Effect::Writes,
            "Bring in an agent from the Claude Code configuration on this machine: its description, instructions and tools, and its model and skills when the library has them. The result lists what could not be kept. An id the library has is refused unless replace is true.",
            move |o, i: ImportInput| {
                let au = a.clone();
                async move {
                    let by = au.by(&o).await?;
                    let written = o.import_agent(&by, &i.name, &i.group, i.replace).await;
                    au.answer(&o, written, |(a, report)| {
                        (
                            format!("Import agent {} from Claude Code.", a.name),
                            format!(
                                "Imported agent {} ({}).{}",
                                a.id,
                                tools_label(a.loadout.tools.as_deref()),
                                report_text(&report)
                            ),
                        )
                    })
                    .await
                }
            },
        );
    }
}
