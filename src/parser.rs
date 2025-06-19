// Parts originally from "ghrepo-rust", MIT License.
//
const DOTGIT: &str = ".git";
const SLASH: &str = "/";

/// Split a string into a maximal prefix of chars that match `pred` and the
/// remainder of the string
fn span<P>(s: &str, mut pred: P) -> (&str, &str)
where
    P: FnMut(char) -> bool,
{
    match s.find(|c| !pred(c)) {
        Some(i) => s.split_at(i),
        None => s.split_at(s.len()),
    }
}

/// If `s` starts with a valid GitHub organization name, return the org and the remainder of `s`.
fn split_org(s: &str) -> Option<(&str, &str)> {
    let (org, rem) = span(s, is_org_char);
    if org.is_empty() || org.eq_ignore_ascii_case("none") {
        None
    } else {
        Some((org, rem))
    }
}

fn is_org_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_'
}

/// If `s` starts with a valid GitHub repository name, return the name and the
/// remainder of `s`.
fn split_name(s: &str) -> Option<(&str, &str)> {
    let (name, rem) = span(s, is_name_char);
    let (name, rem) = match name.len().checked_sub(4) {
        Some(i) if name.get(i..).unwrap_or("").eq_ignore_ascii_case(DOTGIT) => s.split_at(i),
        _ => (name, rem),
    };
    if name.is_empty() || name == "." || name == ".." {
        None
    } else {
        Some((name, rem))
    }
}

fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.'
}

/// If `s` starts with a prefix of the form `ORG/NAME`, where `ORG` is a
/// valid GitHub org and `NAME` is a valid GitHub repository name, return the
/// org, the name, and the remainder of `s`.
fn split_org_name(s: &str) -> Option<(&str, &str, &str)> {
    let (org, s) = split_org(s)?;
    let s = s.strip_prefix(SLASH)?;
    let (name, s) = split_name(s)?;
    Some((org, name, s))
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum State {
    Start,
    Http,
    Web,
    OrgName,
    OrgNameGit,
    End,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum Token {
    /// A string to match exactly
    Literal(&'static str),
    /// A string to match regardless of differences in ASCII case
    CaseFold(&'static str),
}

impl From<&'static str> for Token {
    fn from(s: &'static str) -> Token {
        Token::Literal(s)
    }
}

static START_PATTERNS: &[(&[Token], State)] = &[
    (&[Token::CaseFold("https://")], State::Http),
    (&[Token::CaseFold("http://")], State::Http),
    (&[Token::CaseFold("git://")], State::OrgNameGit),
    (&[Token::Literal("git@")], State::OrgNameGit),
    (
        &[Token::CaseFold("ssh://"), Token::Literal("git@")],
        State::OrgNameGit,
    ),
];

/// If `s` is a valid Git repository URL, return the host, repository org &
/// name.  The following URL formats are recognized:
///
/// - `[http[s]://[<username>[:<password>]@]][www.]<host>/<org>/<name>[.git][/]`
/// - `[http[s]://]api.<host>/repos/<org>/<name>`
/// - `git://<host>/<org>/<name>[.git]`
/// - `git@<host>:<org>/<name>[.git]`
/// - `ssh://git@<host>/<org>/<name>[.git]`
pub fn parse_git_url(s: &str) -> Option<(&str, &str, &str)> {
    // Notes on case sensitivity:
    // - Schemes & hostnames in URLs are case insensitive per RFC 3986 (though
    //   `git clone` as of Git 2.38.1 doesn't actually accept non-lowercase
    //   schemes).
    // - The "repos" in an API URL is case sensitive; changing the case results
    //   in a 404.
    // - The "git" username in SSH URLs (both forms) is case sensitive;
    //   changing the case results in a permissions error.
    // - The optional ".git" suffix is case sensitive; changing the case (when
    //   cloning with `git clone`, at least) results in either a credentials
    //   prompt for HTTPS URLs (the same as if you'd specified a nonexistent
    //   repo) or a "repository not found" message for SSH URLs.
    let mut parser = PullParser::new(s);
    let mut state = State::Start;
    let mut host: Option<&str> = None;
    let mut result: Option<(&str, &str)> = None;
    loop {
        state = match state {
            State::Start => START_PATTERNS
                .iter()
                .find_map(|&(tokens, transition)| parser.consume_seq(tokens).and(Some(transition)))
                .unwrap_or(State::Web),
            State::Http => {
                parser.maybe_consume_userinfo();
                let parsed_host = parser.consume_host()?;
                host = Some(parsed_host);

                match parser
                    .consume("/repos/".into())
                    .or_else(|| parser.consume(SLASH.into()))
                {
                    Some(()) => State::OrgName,
                    None => return None,
                }
            }
            State::Web => {
                parser.maybe_consume(Token::CaseFold("www."));
                let parsed_host = parser.consume_host()?;
                host = Some(parsed_host);
                parser.consume(SLASH.into())?;
                result = Some(parser.get_org_name()?);
                parser.maybe_consume(DOTGIT.into());
                parser.maybe_consume(SLASH.into());
                State::End
            }
            State::OrgName => {
                result = Some(parser.get_org_name()?);
                parser.maybe_consume(DOTGIT.into());
                parser.maybe_consume(SLASH.into());
                State::End
            }
            State::OrgNameGit => {
                let parsed_host = parser.consume_host()?;
                host = Some(parsed_host);
                if parser.consume(":".into()).is_some() || parser.consume(SLASH.into()).is_some() {
                    result = Some(parser.get_org_name()?);
                    parser.maybe_consume(DOTGIT.into());
                    State::End
                } else {
                    return None;
                }
            }
            State::End => {
                return if parser.at_end() {
                    match (host, result) {
                        (Some(h), Some((org, name))) => Some((h, org, name)),
                        _ => None,
                    }
                } else {
                    None
                };
            }
        }
    }
}

struct PullParser<'a> {
    data: &'a str,
}

impl<'a> PullParser<'a> {
    fn new(data: &'a str) -> Self {
        Self { data }
    }

    fn consume_seq<'b, I>(&mut self, tokens: I) -> Option<()>
    where
        I: IntoIterator<Item = &'b Token>,
    {
        let orig = self.data;
        for &t in tokens {
            if self.consume(t).is_none() {
                self.data = orig;
                return None;
            }
        }
        Some(())
    }

    fn consume(&mut self, token: Token) -> Option<()> {
        match token {
            Token::Literal(s) => match self.data.strip_prefix(s) {
                Some(t) => {
                    self.data = t;
                    Some(())
                }
                None => None,
            },
            Token::CaseFold(s) => {
                let i = s.len();
                match self.data.get(..i).zip(self.data.get(i..)) {
                    Some((t, u)) if t.eq_ignore_ascii_case(s) => {
                        self.data = u;
                        Some(())
                    }
                    _ => None,
                }
            }
        }
    }

    fn maybe_consume(&mut self, token: Token) {
        let _ = self.consume(token);
    }

    fn get_org_name(&mut self) -> Option<(&'a str, &'a str)> {
        let (org, name, s) = split_org_name(self.data)?;
        self.data = s;
        Some((org, name))
    }

    fn consume_host(&mut self) -> Option<&'a str> {
        let (host, remainder) = span(self.data, is_hostname_char);
        if is_valid_hostname(host) {
            self.data = remainder;
            Some(host)
        } else {
            None
        }
    }

    /// If the current state starts with a (possibly empty) URL userinfo field
    /// followed by a `@`, consume them both.
    fn maybe_consume_userinfo(&mut self) {
        // cf. <https://datatracker.ietf.org/doc/html/rfc3986#section-3.2.1>
        if let Some((userinfo, s)) = self.data.split_once('@') {
            if userinfo.chars().all(is_userinfo_char) {
                self.data = s;
            }
        }
    }

    fn at_end(&self) -> bool {
        self.data.is_empty()
    }
}

fn is_userinfo_char(c: char) -> bool {
    // RFC 3986 requires that percent signs be followed by two hex digits, but
    // we're not going to bother enforcing that.
    c.is_ascii_alphanumeric() || "-._~!$&'()*+,;=%:".contains(c)
}

fn is_hostname_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '.'
}

fn is_valid_hostname(hostname: &str) -> bool {
    if hostname.is_empty() || hostname.len() > 253 {
        return false;
    }

    // Split by dots and validate each label
    for label in hostname.split('.') {
        if label.is_empty() || label.len() > 63 {
            return false;
        }

        // Labels cannot start or end with hyphens
        if label.starts_with('-') || label.ends_with('-') {
            return false;
        }

        // Labels must contain only alphanumeric characters and hyphens
        if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_git_url_https() {
        // Test various hosts with HTTPS
        assert_eq!(
            parse_git_url("https://github.com/org/repo"),
            Some(("github.com", "org", "repo"))
        );
        assert_eq!(
            parse_git_url("https://gitlab.com/org/repo"),
            Some(("gitlab.com", "org", "repo"))
        );
        assert_eq!(
            parse_git_url("https://bitbucket.org/org/repo.git"),
            Some(("bitbucket.org", "org", "repo"))
        );
        assert_eq!(
            parse_git_url("https://git.example.com/org/repo"),
            Some(("git.example.com", "org", "repo"))
        );
    }

    #[test]
    fn test_parse_git_url_ssh() {
        // Test SSH format
        assert_eq!(
            parse_git_url("git@github.com:org/repo.git"),
            Some(("github.com", "org", "repo"))
        );
        assert_eq!(
            parse_git_url("git@gitlab.com:org/repo"),
            Some(("gitlab.com", "org", "repo"))
        );
        assert_eq!(
            parse_git_url("git@git.example.com:org/repo.git"),
            Some(("git.example.com", "org", "repo"))
        );
    }

    #[test]
    fn test_parse_git_url_git_protocol() {
        // Test git:// protocol
        assert_eq!(
            parse_git_url("git://github.com/org/repo.git"),
            Some(("github.com", "org", "repo"))
        );
        assert_eq!(
            parse_git_url("git://gitlab.com/org/repo"),
            Some(("gitlab.com", "org", "repo"))
        );
    }

    #[test]
    fn test_parse_git_url_ssh_full() {
        // Test full SSH format
        assert_eq!(
            parse_git_url("ssh://git@github.com/org/repo.git"),
            Some(("github.com", "org", "repo"))
        );
        assert_eq!(
            parse_git_url("ssh://git@gitlab.com/org/repo"),
            Some(("gitlab.com", "org", "repo"))
        );
    }

    #[test]
    fn test_parse_git_url_invalid() {
        // Test invalid URLs
        assert_eq!(parse_git_url("not-a-url"), None);
        assert_eq!(parse_git_url("https://"), None);
        assert_eq!(parse_git_url("https://example.com"), None);
        assert_eq!(parse_git_url(""), None);
    }

    #[test]
    fn test_hostname_validation() {
        assert!(is_valid_hostname("github.com"));
        assert!(is_valid_hostname("gitlab.com"));
        assert!(is_valid_hostname("git.example.com"));
        assert!(is_valid_hostname("localhost"));
        assert!(is_valid_hostname("example-host.com"));

        assert!(!is_valid_hostname(""));
        assert!(!is_valid_hostname("-invalid.com"));
        assert!(!is_valid_hostname("invalid-.com"));
        // Create a hostname longer than 253 characters
        let long_hostname = "a".repeat(254);
        assert!(!is_valid_hostname(&long_hostname));
    }
}
