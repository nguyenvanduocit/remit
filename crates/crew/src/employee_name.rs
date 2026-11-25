//! An employee's own name (ADR-0061). Every employee has its number (`Employee::code`); an agent may add a name it
//! chooses, once, through name_yourself, and then it is fixed. A person's name is the one the owner gave. Nothing waits
//! on a name.

use std::sync::Arc;

use model::{Card, Employee};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::Crew;
use crate::employee_face::face_of;
use chains::tools::{Effect, Tools};
use chains::{Error, Result};
use wire::app::EmployeeRead;

/// e as a channel reads it: how the server words it (`Employee::shown` and its tag), and its face (employee_face.rs).
pub fn employee_read_of(e: &Employee) -> EmployeeRead {
    EmployeeRead {
        employee: e.clone(),
        shown: e.shown(),
        tag: e.tag(),
        face: face_of(e),
    }
}

/// The most characters a chosen name holds.
const NAME_MAX: usize = 24;

/// raw as an employee's name, trimmed, or why it cannot be one: a word or two of letters (any script), with hyphens and
/// apostrophes inside a word, at most `NAME_MAX` characters, and no other employee's name in staff, retired ones
/// included, whatever the case. Pure.
pub(crate) fn chosen_name(raw: &str, staff: &[Employee]) -> Result<String> {
    let words: Vec<&str> = raw.split_whitespace().collect();
    let name = words.join(" ");
    let n = name.chars().count();
    if n == 0 {
        return Err(Error::refused(
            "a name is a word or two of letters, like Mai or Anne-Marie: choose one",
        ));
    }
    if n > NAME_MAX {
        return Err(Error::refused(format!(
            "{name:?} is longer than {NAME_MAX} characters: choose a shorter name"
        )));
    }
    if words.len() > 2 {
        return Err(Error::refused(format!(
            "{name:?} is more than two words: a name is a word or two of letters, like Mai or Anne-Marie"
        )));
    }
    if !words.iter().all(|w| name_word(w)) {
        return Err(Error::refused(format!(
            "{name:?} is not a name: use letters, with a hyphen or an apostrophe inside a word, like Mai or Anne-Marie"
        )));
    }
    let folded = name.to_lowercase();
    if let Some(e) = staff.iter().find(|e| e.label.to_lowercase() == folded) {
        return Err(Error::refused(format!(
            "{name} is taken, by {}: choose another name",
            e.code()
        )));
    }
    Ok(name)
}

/// Whether w is letters, with hyphens and apostrophes only between them.
fn name_word(w: &str) -> bool {
    let runes: Vec<char> = w.chars().collect();
    runes.iter().enumerate().all(|(i, r)| {
        if r.is_alphabetic() {
            return true;
        }
        let joiner = matches!(r, '-' | '\'' | '’');
        let letter_at = |k: Option<usize>| k.and_then(|k| runes.get(k)).is_some_and(|c| c.is_alphabetic());
        joiner && letter_at(i.checked_sub(1)) && letter_at(i.checked_add(1))
    })
}

/// The input of name_yourself.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct NameInput {
    /// the name you choose for yourself: a given name, a word or two of letters, like Mai or Anne-Marie, at most 24
    /// characters, no other employee's
    name: String,
}

impl Crew {
    /// Gives an agent employee with no name of its own name_yourself.
    pub async fn add_name_tool(self: &Arc<Self>, t: &mut Tools, card: &Card) {
        if card.employee.is_empty() {
            return;
        }
        let id = card.employee.clone();
        let Ok(emp) = self.store.read(move |r| r.employee(&id)).await else {
            return;
        };
        if !emp.label.is_empty() {
            return;
        }
        let description = format!(
            "You may choose a name of your own, once: it is yours for as long as you are kept, beside your number {}, and \
             fixed once chosen. A given name, a word or two of letters, no other employee's. Nothing waits on it.",
            emp.code()
        );
        let id = emp.id;
        t.serving(self).add(
            "name_yourself",
            Effect::Writes,
            description,
            move |o: Arc<Crew>, input: NameInput| {
                let id = id.clone();
                async move {
                    let named = o.name_yourself(&id, &input.name).await?;
                    Ok(format!("You are {}.", named.shown()))
                }
            },
        );
    }

    /// Gives employee id the name it chose, once: a second name is refused, as is one `chosen_name` refuses.
    pub(crate) async fn name_yourself(&self, id: &str, raw: &str) -> Result<Employee> {
        let (id, raw) = (id.to_owned(), raw.to_owned());
        let named = self
            .store
            .update(move |tx| -> Result<Employee> {
                let mut emp = tx.employee(&id)?;
                if !emp.label.is_empty() {
                    return Err(Error::refused(format!(
                        "you are {} already: a name is chosen once",
                        emp.label
                    )));
                }
                emp.label = chosen_name(&raw, &tx.employees()?)?;
                tx.put_employee(emp)?;
                Ok(tx.employee(&id)?)
            })
            .await?;
        self.notify.employee_updated(&named);
        Ok(named)
    }
}
