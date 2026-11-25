//! Importing brings in what the Claude Code configuration on this machine already holds. It reads only, from
//! `Config::claude_config_dir`, and each import goes through the same save as a record an agent wrote: the library
//! validates it the same way. Agents can write that configuration, so an agent's import waits for the owner where its
//! write would (rail.rs).

use model::{Actor, Agent, Skill};

use crate::Capabilities;
use crate::over::Over;
use chains::chain_core::optional;
use chains::{Error, Result};

/// Why an import stops at what is there: it is the owner's until he says to replace it.
const ALREADY_THERE: &str = "already exists: import it again with replace set to true to overwrite it";

impl Capabilities {
    /// What the configuration holds that an import could take.
    pub(crate) async fn importable(&self) -> Result<Vec<importer::Candidate>> {
        let dir = self.cfg.claude_config_dir.clone();
        tokio::task::spawn_blocking(move || importer::list(&dir))
            .await
            .map_err(|e| Error::refused(format!("the Claude Code configuration: {e}")))?
            .map_err(|e| Error::refused(e.to_string()))
    }

    /// Imports skills/<name> into group. A skill the library already has is refused unless replace says to overwrite it.
    pub(crate) async fn import_skill(
        &self,
        by: &Actor,
        name: &str,
        group: &str,
        replace: bool,
    ) -> Result<(Skill, importer::Report)> {
        let (dir, key, at) = (self.cfg.claude_config_dir.clone(), name.to_owned(), self.now());
        let (mut skill, report) = tokio::task::spawn_blocking(move || importer::skill(&dir, &key, at))
            .await
            .map_err(|e| Error::refused(format!("the Claude Code configuration: {e}")))?
            .map_err(|e| Error::refused(e.to_string()))?;
        group.clone_into(&mut skill.group);
        if !replace {
            let key = skill.name.clone();
            if optional(self.store.read(move |r| r.skill(&key)).await)?.is_some() {
                return Err(Error::refused(format!("skill {:?}: {ALREADY_THERE}", skill.name)));
            }
        }
        self.hold_skill(by, &skill).await?;
        let skill = self.save_skill(by, skill, replacing(replace)).await?;
        Ok((skill, report))
    }

    /// Imports agents/<name>.md into group. The record keeps only the models and skills the library has. An agent the
    /// library already has is refused unless replace says to overwrite it.
    pub(crate) async fn import_agent(
        &self,
        by: &Actor,
        name: &str,
        group: &str,
        replace: bool,
    ) -> Result<(Agent, importer::Report)> {
        let held = self.library_of().await?;
        let known = importer::Known {
            models: self
                .cfg
                .models
                .as_ref()
                .map(|m| m.models().iter().map(|m| m.id.clone()).collect())
                .unwrap_or_default(),
            skills: held.skills.into_iter().map(|s| s.name).collect(),
        };
        let (dir, key, at) = (self.cfg.claude_config_dir.clone(), name.to_owned(), self.now());
        let (mut agent, report) = tokio::task::spawn_blocking(move || importer::agent(&dir, &key, at, &known))
            .await
            .map_err(|e| Error::refused(format!("the Claude Code configuration: {e}")))?
            .map_err(|e| Error::refused(e.to_string()))?;
        group.clone_into(&mut agent.group);
        if !replace {
            let key = agent.id.clone();
            if optional(self.store.read(move |r| r.agent(&key)).await)?.is_some() {
                return Err(Error::refused(format!("agent {:?}: {ALREADY_THERE}", agent.id)));
            }
        }
        self.hold_agent(by, &agent).await?;
        let agent = self.save_agent(by, agent, replacing(replace)).await?;
        Ok((agent, report))
    }
}

/// What an import is saved over: nothing, or what is stored when replace says to overwrite it. Pure.
fn replacing(replace: bool) -> Over {
    if replace { Over::Stored } else { Over::New }
}
