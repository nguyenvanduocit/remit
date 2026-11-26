//! The catalog of the models a card can run on, all of them Claude Code's, and the efforts it can run them at. A card
//! stores only its model id and effort; how capable a model is, and which one a health check runs on, are read from here.

use std::fmt;

/// How capable a model is, the least capable first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Tier {
    Fast,
    Workhorse,
    Frontier,
    Apex,
}

impl fmt::Display for Tier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Fast => "fast",
            Self::Workhorse => "workhorse",
            Self::Frontier => "frontier",
            Self::Apex => "apex",
        })
    }
}

impl Tier {
    /// The work this tier is for, in words a decider reads to choose between tiers (ADR-0118).
    #[must_use]
    pub const fn fits(self) -> &'static str {
        match self {
            Self::Fast => {
                "plain, well-specified work whose result a check proves: collecting facts, a small mechanical edit, formatting"
            }
            Self::Workhorse => {
                "everyday work with a clear goal: a feature or fix in a known codebase, research with sources, routine review"
            }
            Self::Frontier => {
                "hard work: a design across modules, a subtle bug, work with several parts no check can prove"
            }
            Self::Apex => {
                "the hardest work, where a mistake is costly: architecture, security-sensitive changes, judgement others rely on"
            }
        }
    }
}

/// One model a card can run on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    /// What a card's model field holds and what the Chair names when it hires.
    pub id: String,
    pub tier: Tier,
}

/// The models Remit can run, in the order it prefers them when two are as good a choice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Catalog(Vec<Model>);

/// Every model Remit knows.
impl Default for Catalog {
    fn default() -> Self {
        Self(
            [
                ("fable", Tier::Apex),
                ("opus", Tier::Frontier),
                ("sonnet", Tier::Workhorse),
                ("haiku", Tier::Fast),
            ]
            .into_iter()
            .map(|(id, tier)| Model {
                id: id.to_owned(),
                tier,
            })
            .collect(),
        )
    }
}

/// A catalog of the models a caller names, an empty one included: a server configured with no models offers none.
impl From<Vec<Model>> for Catalog {
    fn from(models: Vec<Model>) -> Self {
        Self(models)
    }
}

impl Catalog {
    /// The models in the catalog's order.
    #[must_use]
    pub fn models(&self) -> &[Model] {
        &self.0
    }

    /// The models and what each is, as an agent that picks one reads them: the id to name and how capable it is. Empty
    /// for an empty catalog.
    #[must_use]
    pub fn describe(&self) -> String {
        let items: Vec<String> = self.0.iter().map(|m| format!("{} ({})", m.id, m.tier)).collect();
        items.join(", ")
    }

    /// The model with `id`.
    #[must_use]
    pub fn lookup(&self, id: &str) -> Option<&Model> {
        self.0.iter().find(|m| m.id == id)
    }

    /// The model of the lowest tier, the first listed of two as low: the quickest to prove an account answers. None for
    /// an empty catalog.
    #[must_use]
    pub fn fastest(&self) -> Option<&Model> {
        // `min_by_key` keeps the first of equal minima, which is the order of preference.
        self.0.iter().min_by_key(|m| m.tier)
    }

    /// The model a card of `tier` climbs to: the first listed of the next tier the catalog holds above it. None when the
    /// catalog holds none above.
    #[must_use]
    pub fn above(&self, tier: Tier) -> Option<&Model> {
        let next = self.0.iter().map(|m| m.tier).filter(|t| *t > tier).min()?;
        self.0.iter().find(|m| m.tier == next)
    }

    /// The first listed model of `tier`. None when the catalog holds none.
    #[must_use]
    pub fn of(&self, tier: Tier) -> Option<&Model> {
        self.0.iter().find(|m| m.tier == tier)
    }

    /// The catalog without the models above `top`'s tier: the most capable model the owner lets Remit run (`-top-model`).
    /// None when the catalog does not offer `top`.
    #[must_use]
    pub fn up_to(&self, top: &str) -> Option<Self> {
        let ceiling = self.lookup(top)?.tier;
        Some(Self(self.0.iter().filter(|m| m.tier <= ceiling).cloned().collect()))
    }

    /// The model of the highest tier, the first listed of two as high: what every judge runs on, so no author it grades
    /// runs on a stronger one (ADR-0102). None for an empty catalog.
    #[must_use]
    pub fn strongest(&self) -> Option<&Model> {
        // `max_by_key` keeps the last of equal maxima; reversed, it keeps the first, which is the order of preference.
        self.0.iter().rev().max_by_key(|m| m.tier)
    }
}

/// How long a model thinks before it acts, as Claude Code's `--effort` takes it. Every catalog model takes every level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Effort {
    Low,
    Medium,
    High,
    XHigh,
    Max,
}

impl Effort {
    /// Every level, the shortest thinking first.
    pub const ALL: [Self; 5] = [Self::Low, Self::Medium, Self::High, Self::XHigh, Self::Max];

    /// The work this level is for, in words a decider reads to choose between levels (ADR-0118).
    #[must_use]
    pub const fn fits(self) -> &'static str {
        match self {
            Self::Low => "nothing to think through: the task is plain and its steps are given",
            Self::Medium => "a clear task with a few decisions to make",
            Self::High => "work that is hard to get right, or that touches many things",
            Self::XHigh => "very hard work with many interacting parts, where a slip is easy to make",
            Self::Max => "the hardest problems, where every further pass of thinking pays",
        }
    }

    /// The word a record and Claude Code's command line hold.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::XHigh => "xhigh",
            Self::Max => "max",
        }
    }

    /// The level `s` names: none for empty, which keeps Claude Code's default; anything else is refused with the list.
    ///
    /// # Errors
    /// `s` is neither empty nor one of the five words.
    pub fn parse(s: &str) -> Result<Option<Self>, String> {
        if s.is_empty() {
            return Ok(None);
        }
        Self::ALL
            .into_iter()
            .find(|e| e.as_str() == s)
            .map(Some)
            .ok_or_else(|| format!("{s:?} is not an effort: choose one of {}", Self::list()))
    }

    /// The levels as an agent that picks one reads them.
    #[must_use]
    pub fn list() -> String {
        let words: Vec<&str> = Self::ALL.iter().map(|e| e.as_str()).collect();
        words.join(", ")
    }
}

impl fmt::Display for Effort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
