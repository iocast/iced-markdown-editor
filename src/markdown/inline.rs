//! Export of inline content: delimiters, escaping, links, images, code spans.

use super::{flag_tag, is_break, parse};
use crate::doc::{Block, Flag, Link, OBJ, SOFT, Style};

const MARKS: [Flag; 3] = [Flag::Bold, Flag::Italic, Flag::Strike];

fn mark_token(flag: Flag, use_html: bool, closing: bool) -> &'static str {
    if use_html {
        flag_tag(flag, closing)
    } else {
        match flag {
            Flag::Bold => "**",
            Flag::Italic => "*",
            Flag::Strike => "~~",
            Flag::Code => "`",
        }
    }
}

fn esc_text(c: char, line_start: bool, heading: bool) -> String {
    if heading && c == '#' {
        return "\\#".into();
    }
    if "\\`*_[]<>~&|!".contains(c) {
        return format!("\\{c}");
    }
    if line_start && "#>+-=".contains(c) {
        return format!("\\{c}");
    }
    c.to_string()
}

fn esc_dest(url: &str) -> String {
    // Plain form when nothing needs escaping, `<...>` otherwise.
    let plain = !url.is_empty()
        && !url
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || "<>()\\&".contains(c));
    if plain {
        return url.to_string();
    }
    let mut out = String::from("<");
    for c in url.chars() {
        match c {
            '<' | '>' | '\\' | '&' => {
                out.push('\\');
                out.push(c);
            }
            '\n' | '\r' => out.push(' '),
            c => out.push(c),
        }
    }
    out.push('>');
    out
}

fn esc_title(title: &str) -> String {
    let mut out = String::from("\"");
    for c in title.chars() {
        if matches!(c, '"' | '\\' | '&') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    out
}

fn code_span(text: &str) -> String {
    let mut longest = 0;
    let mut run = 0;
    for c in text.chars() {
        run = if c == '`' { run + 1 } else { 0 };
        longest = longest.max(run);
    }
    let ticks = "`".repeat(longest + 1);
    let pad = text.starts_with('`')
        || text.ends_with('`')
        || (text.starts_with(' ') && text.ends_with(' ') && !text.trim_matches(' ').is_empty());
    if pad {
        format!("{ticks} {text} {ticks}")
    } else {
        format!("{ticks}{text}{ticks}")
    }
}

/// `scheme:...` usable as an autolink.
fn is_autolink(url: &str) -> bool {
    let Some((scheme, rest)) = url.split_once(':') else {
        return false;
    };
    let mut chars = scheme.chars();
    let first_ok = chars.next().is_some_and(|c| c.is_ascii_alphabetic());
    let len_ok = (2..=32).contains(&scheme.len());
    let scheme_ok = scheme
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'));
    let rest_ok = rest
        .chars()
        .all(|c| !c.is_whitespace() && !c.is_control() && c != '<' && c != '>');
    first_ok && len_ok && scheme_ok && rest_ok
}

fn is_email(text: &str) -> bool {
    let Some((local, domain)) = text.split_once('@') else {
        return false;
    };
    let local_ok = !local.is_empty()
        && local
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || ".!#$%&'*+/=?^_`{|}~-".contains(c));
    let domain_ok = !domain.is_empty()
        && domain.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
                && !label.starts_with('-')
                && !label.ends_with('-')
        });
    local_ok && domain_ok
}

fn image_md(alt: &[char], target: &Link) -> String {
    let mut text = String::new();
    for c in alt {
        match *c {
            OBJ => {}
            SOFT | '\n' => text.push(' '),
            c => text.push_str(&esc_text(c, false, false)),
        }
    }
    let title = target
        .title
        .as_deref()
        .map(|title| format!(" {}", esc_title(title)))
        .unwrap_or_default();
    format!("![{text}]({}{title})", esc_dest(&target.url))
}

/// `![alt](url)` with escaping, for inserting images as Markdown text.
pub(crate) fn image_markdown(alt: &str, url: &str) -> String {
    let alt: Vec<char> = alt.chars().collect();
    image_md(
        &alt,
        &Link {
            url: url.to_string(),
            title: None,
        },
    )
}

struct Inline<'a> {
    chars: &'a [char],
    styles: &'a [Style],
    heading: bool,
    use_html: bool,
    para_start: bool,
    para_end: bool,
    out: String,
    open: Vec<Flag>,
    line_start: bool,
}

impl<'a> Inline<'a> {
    fn has(&self, k: usize, flag: Flag) -> bool {
        self.styles[k].get(flag)
    }

    fn wanted(&self, k: usize) -> Vec<Flag> {
        MARKS
            .into_iter()
            .filter(|flag| self.has(k, *flag))
            .collect()
    }

    fn mark_end(&self, flag: Flag, mut k: usize) -> usize {
        while k < self.chars.len() && self.has(k, flag) {
            k += 1;
        }
        k
    }

    /// Closes every open mark not in `keep` (and everything opened after it).
    fn close_to(&mut self, keep: &[Flag]) {
        while self.open.iter().any(|flag| !keep.contains(flag)) {
            if let Some(flag) = self.open.pop() {
                self.out.push_str(mark_token(flag, self.use_html, true));
            }
        }
    }

    /// Opens `marks` at `k`; the longest-running mark becomes outermost.
    fn open_at(&mut self, k: usize, marks: &[Flag]) {
        let mut new: Vec<(Flag, usize)> = marks
            .iter()
            .filter(|flag| !self.open.contains(flag))
            .map(|flag| (*flag, self.mark_end(*flag, k)))
            .collect();
        new.sort_by(|a, b| b.1.cmp(&a.1));
        for (flag, _) in new {
            self.open.push(flag);
            self.out.push_str(mark_token(flag, self.use_html, false));
        }
    }

    fn emit_text(&mut self, text: &[char], at_end: bool) {
        let mut text: Vec<char> = text.to_vec();
        let mut tail = String::new();
        if at_end {
            while let Some(c) = text.last().copied() {
                if c == ' ' || c == '\t' {
                    tail.insert_str(0, if c == ' ' { "&#32;" } else { "&#9;" });
                    text.pop();
                } else {
                    break;
                }
            }
        }

        let mut k = 0;
        while k < text.len() {
            let c = text[k];
            if c == OBJ {
                k += 1;
                continue;
            }
            if self.line_start && (c == ' ' || c == '\t') {
                self.out.push_str(if c == ' ' { "&#32;" } else { "&#9;" });
                k += 1;
                continue;
            }
            if self.line_start && c.is_ascii_digit() {
                let mut m = k;
                while m < text.len() && text[m].is_ascii_digit() {
                    m += 1;
                }
                if m < text.len() && (text[m] == '.' || text[m] == ')') {
                    self.out.extend(&text[k..m]);
                    self.out.push('\\');
                    self.out.push(text[m]);
                    k = m + 1;
                    self.line_start = false;
                    continue;
                }
            }
            if c == '\u{a0}' {
                self.out.push_str("&nbsp;");
            } else {
                let escaped = esc_text(c, self.line_start, self.heading);
                self.out.push_str(&escaped);
            }
            self.line_start = false;
            k += 1;
        }

        if !tail.is_empty() {
            self.out.push_str(&tail);
            self.line_start = false;
        }
    }

    fn run(mut self) -> String {
        let n = self.chars.len();
        let mut i = 0;

        while i < n {
            let c = self.chars[i];
            let style = self.styles[i].clone();

            if is_break(c) {
                if style.html && c == SOFT {
                    // A line ending inside raw HTML.
                    self.out.push('\n');
                    self.line_start = true;
                    i += 1;
                    continue;
                }
                let trailing = self.chars[i + 1..]
                    .iter()
                    .all(|ch| *ch == '\n' || *ch == OBJ);
                if c == '\n' && ((i == 0 && self.para_start) || (self.para_end && trailing)) {
                    // A hard break at the edge of a paragraph cannot be expressed.
                    i += 1;
                    continue;
                }
                let prev = if i > 0 { Some(self.chars[i - 1]) } else { None };
                let next = self.chars.get(i + 1).copied();
                let edge = (prev.is_none() && self.para_start)
                    || (next.is_none() && self.para_end)
                    || prev.is_some_and(is_break)
                    || next.is_some_and(is_break);
                if c == SOFT && (self.heading || edge) {
                    self.out.push_str("&#10;");
                    self.line_start = false;
                } else if self.heading {
                    self.out.push('\n');
                    self.line_start = true;
                } else {
                    self.out.push_str(if c == '\n' { "\\\n" } else { "\n" });
                    self.line_start = true;
                }
                i += 1;
                continue;
            }

            if let Some(link) = style.link.clone() {
                let mut j = i;
                while j < n && self.styles[j].link.as_ref() == Some(&link) {
                    j += 1;
                }
                // Marks covering the whole link and continuing outside it
                // are written around the link, others inside.
                let after = if j < n { self.wanted(j) } else { Vec::new() };
                let outside: Vec<Flag> = self
                    .wanted(i)
                    .into_iter()
                    .filter(|flag| {
                        (self.open.contains(flag) || after.contains(flag))
                            && (i..j).all(|k| self.has(k, *flag))
                    })
                    .collect();
                self.close_to(&outside);
                self.open_at(i, &outside);

                let text: String = self.chars[i..j].iter().collect();
                let plain_style = Style {
                    bold: outside.contains(&Flag::Bold),
                    italic: outside.contains(&Flag::Italic),
                    strike: outside.contains(&Flag::Strike),
                    link: Some(link.clone()),
                    ..Style::default()
                };
                let plain = self.styles[i..j].iter().all(|st| *st == plain_style);
                if plain && link.title.is_none() && link.url == text && is_autolink(&link.url) {
                    self.out.push_str(&format!("<{}>", link.url));
                } else if plain
                    && link.title.is_none()
                    && link.url == format!("mailto:{text}")
                    && is_email(&text)
                {
                    self.out.push_str(&format!("<{text}>"));
                } else {
                    let inner: Vec<Style> = self.styles[i..j]
                        .iter()
                        .map(|st| {
                            let mut st = st.clone();
                            st.link = None;
                            for flag in &outside {
                                st.set(*flag, false);
                            }
                            st
                        })
                        .collect();
                    let body = Inline {
                        chars: &self.chars[i..j],
                        styles: &inner,
                        heading: self.heading,
                        use_html: self.use_html,
                        para_start: false,
                        para_end: false,
                        out: String::new(),
                        open: Vec::new(),
                        line_start: true,
                    }
                    .run();
                    let title = link
                        .title
                        .as_deref()
                        .map(|title| format!(" {}", esc_title(title)))
                        .unwrap_or_default();
                    self.out
                        .push_str(&format!("[{body}]({}{title})", esc_dest(&link.url)));
                }
                self.line_start = false;
                i = j;
                continue;
            }

            let wanted = self.wanted(i);
            self.close_to(&wanted);
            self.open_at(i, &wanted);

            let mut j = i;
            if let Some(image) = style.image.clone() {
                while j < n && self.styles[j] == style {
                    j += 1;
                }
                self.out.push_str(&image_md(&self.chars[i..j], &image));
                self.line_start = false;
            } else if style.html {
                while j < n && self.styles[j] == style && self.chars[j] != SOFT {
                    j += 1;
                }
                self.out.extend(&self.chars[i..j]);
                self.line_start = false;
            } else if style.code {
                while j < n && self.styles[j] == style && !is_break(self.chars[j]) {
                    j += 1;
                }
                let text: String = self.chars[i..j].iter().collect();
                self.out.push_str(&code_span(&text));
                self.line_start = false;
            } else {
                while j < n && self.styles[j] == style && !is_break(self.chars[j]) {
                    j += 1;
                }
                let rest = &self.chars[j..];
                let at_end = rest.iter().all(|ch| *ch == OBJ)
                    || rest.first().is_some_and(|ch| is_break(*ch));
                let chars = self.chars;
                self.emit_text(&chars[i..j], at_end);
            }
            i = j;
        }

        self.close_to(&[]);
        self.out
    }
}

pub(super) fn inline_md(chars: &[char], styles: &[Style], heading: bool, use_html: bool) -> String {
    Inline {
        chars,
        styles,
        heading,
        use_html,
        para_start: true,
        para_end: true,
        out: String::new(),
        open: Vec::new(),
        line_start: true,
    }
    .run()
}

/// Markdown delimiters when they survive a round trip, otherwise HTML tags.
pub(super) fn inline_checked(
    block: &Block,
    heading: bool,
    wrap: impl Fn(&str) -> String,
) -> String {
    let mut text = String::new();
    for use_html in [false, true] {
        text = inline_md(&block.chars, &block.styles, heading, use_html);
        if block.chars.is_empty() {
            return text;
        }
        let back = parse(&wrap(&text));
        if back.blocks.len() == 1
            && back.blocks[0].chars == block.chars
            && back.blocks[0].styles == block.styles
        {
            return text;
        }
    }
    text
}
