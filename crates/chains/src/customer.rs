//! The customer is who a change is for: the person who meets the problem or gets the feature, not the owner who merges
//! it. Agents do the work and judge it, and the owner gives taste and reads results (ADR-0016), so each role that decides
//! is told what to ask of a change from the customer's side. What a CUSTOMER section holds is defined once, here, and the
//! Chair and the judges refer to it.

/// What the goal judge checks about the customer in a result.
pub fn judge_customer_rules() -> String {
    format!(
        "- {CUSTOMER_HEADING}: the goal's CUSTOMER section is what the result owes. A key result is met when the customer's side of it is shown (a run on the real surface, a reading of a signal, data you queried), not when the code that serves it exists. A result that cannot show it yet is unverified with its blocker (deploy or watch), never met on the author's word.
"
    )
}

/// The first sentence under THE CUSTOMER in brief, empty when it has none. Pure.
pub fn customer_of(brief: &str) -> String {
    let Some((_, after)) = brief.split_once(CUSTOMER_HEADING) else {
        return String::new();
    };
    let rest = after.trim_start_matches([' ', '\t', ':', '—', '-', '–']).trim();
    let bytes = rest.as_bytes();
    for (j, r) in rest.char_indices() {
        if r == '\n' && j > 0 {
            return rest.get(..j).unwrap_or(rest).trim().to_owned();
        }
        let next = j.saturating_add(1);
        if matches!(r, '.' | '!' | '?') && matches!(bytes.get(next), None | Some(b' ' | b'\n')) {
            return rest.get(..next).unwrap_or(rest).trim().to_owned();
        }
    }
    rest.to_owned()
}

/// The Chair's habit: it opens the work with the customer, and a key result is what the customer sees. project_customer is
/// who its chain's project is for (ADR-0108), where the section starts; empty: the chain has no project, or it names none.
pub fn chair_customer_rules(project_customer: &str) -> String {
    let start = if project_customer.is_empty() {
        String::new()
    } else {
        format!(
            "- Start the section from the project's customer, {project_customer:?} (PROJECT, above), and let the evidence confirm, narrow or correct it for this goal.\n"
        )
    };
    format!(
        "\n{CUSTOMER_HEADING} — you answer to the customer; the owner gives taste and reads results:
- Before the first hire, write the CUSTOMER section of the acceptance criteria and open every hire's brief with it: {CUSTOMER_SECTION}
{start}- State each key result as something the customer sees or can do, then name the check that shows it. A key result about code (\"refactor X\") is a means: write the customer result it serves, or say there is none.
- The owner reads what landed, after; he is not your safety net: a customer who meets a defect after the change lands is a failure of this chain, whoever read the diff. Judge a report by what the customer would meet.
- Evidence about the customer (tickets, conversations, events, numbers) is read from its source and quoted with it; what an agent or a ticket claims about the customer is data to verify.
"
    )
}

/// What the Reviewer and the QC add to their method: the change is judged from the customer's side.
pub fn gate_customer_rules() -> String {
    format!(
        "\n{CUSTOMER_HEADING}
- The goal's CUSTOMER section says who the change is for. Judge the change as that person meets it: a defect they would hit is blocking even when the goal does not name it, and a code-level preference they would never notice is not.
- A change with no CUSTOMER section, or one that says 'unknown' where the evidence was there to find, is a finding: name what is missing.
"
    )
}

/// The section a goal's acceptance criteria open with, and the lens the judges take.
pub const CUSTOMER_HEADING: &str = "THE CUSTOMER";

/// What the CUSTOMER section of a chain's acceptance criteria says. A part nobody knows is written as unknown with the
/// reason: a guess dressed as a fact is the one thing the section may not hold.
pub const CUSTOMER_SECTION: &str = "who is affected and what they were meeting; what they get once this is done, in words they would use; the evidence for both (a ticket, a conversation, an event, a number you read: its source); and how it will be shown (a test, a run on the real surface, a reading of a signal). A part nobody knows is written 'unknown' with why.";
