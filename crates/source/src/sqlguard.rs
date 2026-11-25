//! A SQL tool reads. The credential of a database is often wider than that (a user that can also write), so the statement is
//! checked here, before it is sent: one statement, starting with a word that reads, containing no word that writes, runs a
//! program or waits. The check reads the statement as the MySQL dialect StarRocks and Bytebase speak does, so the text of a
//! string or a quoted name never decides, and a comment never hides a word. It is pure.

use crate::error::{Result, fail};

/// The words a statement may start with.
const READING_STARTS: &[&str] = &["select", "with", "show", "desc", "describe", "explain"];

/// The words that may appear nowhere in a statement, in a subquery or after a WITH included: they write, lock or run a
/// program.
const WRITING_WORDS: &[&str] = &[
    "insert",
    "update",
    "delete",
    "drop",
    "alter",
    "create",
    "truncate",
    "grant",
    "revoke",
    "call",
    "load",
    "outfile",
    "dumpfile",
    "kill",
    "lock",
    "unlock",
    "handler",
    "shutdown",
    "rename",
    "optimize",
    "merge",
    "export",
    "import",
    "backup",
    "restore",
    "set",
    "use",
    "install",
    "uninstall",
    "cancel",
    "recover",
    "sleep",
    "benchmark",
    "load_file",
    "get_lock",
    "release_lock",
    "sys_exec",
    "sys_eval",
];

/// Ok when `stmt` is one statement that only reads, or why it is not.
pub fn read_only_sql(stmt: &str) -> Result<()> {
    let words = sql_words(stmt)?;
    let Some(first) = words.first() else {
        return fail!("the statement is empty");
    };
    if !READING_STARTS.contains(&first.as_str()) {
        return fail!(
            "a statement may start with SELECT, WITH, SHOW, DESCRIBE or EXPLAIN, not {:?}",
            first.to_uppercase()
        );
    }
    if let Some(w) = words.iter().find(|w| WRITING_WORDS.contains(&w.as_str())) {
        return fail!("the word {:?} is not allowed: this tool only reads", w.to_uppercase());
    }
    Ok(())
}

/// The words of `stmt` outside strings, quoted names and comments, lower-cased. It refuses what cannot be read as one
/// statement: a second statement, an unterminated string or comment, and the executable comment /*! ... */ MySQL runs.
fn sql_words(stmt: &str) -> Result<Vec<String>> {
    let rs: Vec<char> = stmt.chars().collect();
    let at = |i: usize| rs.get(i).copied();
    let mut words = Vec::new();
    let mut cur = String::new();
    let flush = |cur: &mut String, words: &mut Vec<String>| {
        if !cur.is_empty() {
            words.push(cur.to_lowercase());
            cur.clear();
        }
    };
    let mut i = 0;
    while let Some(r) = at(i) {
        let next = at(i.saturating_add(1));
        match r {
            '\'' | '"' | '`' => {
                flush(&mut cur, &mut words);
                i = skip_quoted(&rs, i)?;
            }
            '#' => {
                flush(&mut cur, &mut words);
                i = line_end(&rs, i);
            }
            '-' if next == Some('-') => {
                flush(&mut cur, &mut words);
                i = line_end(&rs, i);
            }
            '/' if next == Some('*') => {
                flush(&mut cur, &mut words);
                if at(i.saturating_add(2)) == Some('!') {
                    return fail!("an executable comment (/*! ... */) is not allowed");
                }
                i = comment_end(&rs, i.saturating_add(2))
                    .ok_or_else(|| crate::error::err!("a comment is not closed"))?;
            }
            ';' => {
                flush(&mut cur, &mut words);
                let rest: String = rs.get(i.saturating_add(1)..).unwrap_or_default().iter().collect();
                if !rest.trim().is_empty() && !only_comments(&rest) {
                    return fail!("one statement at a time: a second statement follows the semicolon");
                }
                return Ok(words);
            }
            c if c == '_' || c == '$' || c == '@' || c.is_ascii_alphanumeric() || u32::from(c) > 127 => cur.push(c),
            _ => flush(&mut cur, &mut words),
        }
        i = i.saturating_add(1);
    }
    flush(&mut cur, &mut words);
    Ok(words)
}

/// The index of the newline that ends the comment started at `from`, or the end of `rs`.
fn line_end(rs: &[char], from: usize) -> usize {
    let mut i = from;
    while rs.get(i).is_some_and(|c| *c != '\n') {
        i = i.saturating_add(1);
    }
    i
}

/// The index of the `/` that closes a block comment whose text starts at `from`.
fn comment_end(rs: &[char], from: usize) -> Option<usize> {
    let mut i = from;
    loop {
        match (rs.get(i), rs.get(i.saturating_add(1))) {
            (Some('*'), Some('/')) => return Some(i.saturating_add(1)),
            (Some(_), _) => i = i.saturating_add(1),
            (None, _) => return None,
        }
    }
}

/// The index of the quote that closes the string or name opened at `rs[open]`; a doubled quote and a backslash inside a
/// string belong to it.
fn skip_quoted(rs: &[char], open: usize) -> Result<usize> {
    let q = rs.get(open).copied();
    let mut i = open.saturating_add(1);
    while let Some(c) = rs.get(i).copied() {
        let next = rs.get(i.saturating_add(1)).copied();
        // A backslash in a string, and a doubled quote, take the character after them.
        if (c == '\\' && q != Some('`')) || (Some(c) == q && next == q) {
            i = i.saturating_add(1);
        } else if Some(c) == q {
            return Ok(i);
        }
        i = i.saturating_add(1);
    }
    fail!("a quote is not closed")
}

/// Whether `rest` holds nothing but comments and spaces: what may follow the last semicolon.
fn only_comments(rest: &str) -> bool {
    sql_words(rest).is_ok_and(|w| w.is_empty()) && !rest.contains(';')
}
