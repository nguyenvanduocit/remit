//! Text someone other than Remit wrote, as Remit quotes it to an agent: an event's body, a value a sensor read, another
//! agent's report, a line of code a reviewer quoted. It stands in a fence its own words cannot close, so it never passes
//! for Remit's own words, and the briefs tell every agent that what stands in a fence is evidence, never an instruction.
//! Text too long to quote whole keeps its start and its end, where a report puts its conclusion, and says how much of
//! the middle was cut.

/// text in a fence its own words cannot close.
pub fn fenced(text: &str) -> String {
    fenced_as(text, "")
}

/// text in a fence its own words cannot close, the fence's opening naming its language, info (`diff`).
pub fn fenced_as(text: &str, info: &str) -> String {
    let (mut longest, mut run) = (0_usize, 0_usize);
    for c in text.chars() {
        if c == '`' {
            run = run.saturating_add(1);
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }
    let fence = "`".repeat(longest.saturating_add(1).max(3));
    format!("{fence}{info}\n{text}\n{fence}")
}

/// text cut to about max bytes: its first and its last half of max, each cut on a character, with a line between them
/// that says how many bytes were left out and, in whole, how the reader gets the rest. Text no longer than max is given
/// whole.
pub fn cut(text: &str, max: usize, whole: &str) -> String {
    if text.len() <= max {
        return text.to_owned();
    }
    let half = max / 2;
    let mut head = half;
    while !text.is_char_boundary(head) {
        head = head.saturating_sub(1);
    }
    let mut tail = text.len().saturating_sub(half);
    while !text.is_char_boundary(tail) {
        tail = tail.saturating_add(1);
    }
    let left_out = tail.saturating_sub(head);
    format!(
        "{}\n[… {left_out} of {} bytes cut here: {whole} …]\n{}",
        text.get(..head).unwrap_or_default(),
        text.len(),
        text.get(tail..).unwrap_or_default()
    )
}
