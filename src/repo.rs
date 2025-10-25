use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::{fmt, str};

use thiserror::Error;

use crate::parser::parse_git_url;

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("invalid Git repository spec: {0}")]
    Spec(String),

    #[error("Git command exited unsuccessfully: {0}")]
    CommandFailed(ExitStatus),

    #[error("Found a Git repository, but no remote URL is set for '{0}'")]
    NoSuchRemote(String),

    #[error("Failed to decode output from Git command: {0}")]
    InvalidUtf8(#[from] str::Utf8Error),

    #[error(transparent)]
    CouldNotExecute(#[from] std::io::Error),
}

#[derive(Clone, Default)]
pub struct GitRepository {
    pub host: String,
    pub org: String,
    pub name: String,

    pub path: Option<PathBuf>,
}

impl GitRepository {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, RepositoryError> {
        let url = Self::remote(&path, "origin")?;

        let Some((host, org, name)) = parse_git_url(&url) else {
            return Err(RepositoryError::Spec(url));
        };

        Ok(Self {
            host: host.to_string(),
            org: org.to_string(),
            name: name.to_string(),
            path: Some(path.as_ref().to_path_buf()),
        })
    }

    #[allow(dead_code)]
    pub fn from_url(url: &str) -> Result<Self, RepositoryError> {
        //
        let Some((host, org, name)) = parse_git_url(url) else {
            return Err(RepositoryError::Spec(url.to_string()));
        };

        Ok(Self {
            host: host.to_string(),
            org: org.to_string(),
            name: name.to_string(),
            path: None,
        })
    }

    /// Returns the base URL for accessing the repository via the GitHub REST
    /// API; this is a string of the form
    /// `https://api.github.com/repos/{org}/{name}`.
    #[allow(dead_code)]
    pub fn api_url(&self) -> String {
        format!("https://api.{}/repos/{}/{}", self.host, self.org, self.name)
    }

    /// Returns the URL for cloning the repository via the native Git protocol
    #[allow(dead_code)]
    pub fn git_url(&self) -> String {
        format!("git://{}/{}/{}.git", self.host, self.org, self.name)
    }

    /// Returns the URL for the repository's web interface
    pub fn http_url(&self) -> String {
        let branch = self.current_branch();

        let url = format!("https://{}/{}/{}", self.host, self.org, self.name);

        match branch.as_str() {
            "develop" | "main" | "master" => url,
            _ => format!("{url}/tree/{branch}"),
        }
    }

    /// Returns the URL for cloning the repository over SSH
    #[allow(dead_code)]
    pub fn ssh_url(&self) -> String {
        format!("git@{}:{}/{}.git", self.host, self.org, self.name)
    }

    /// Returns the URL for viewing a specific commit
    pub fn commit_url(&self, hash: &str) -> String {
        format!("https://{}/{}/{}/commit/{}", self.host, self.org, self.name, hash)
    }

    /// Returns the URL for viewing a pull request
    pub fn pr_url(&self, pr_number: &str) -> String {
        format!("https://{}/{}/{}/pull/{}", self.host, self.org, self.name, pr_number)
    }

    /// Try to find a PR number from a commit message
    pub fn pr_for_commit(&self, hash: &str) -> Option<String> {
        if let Some(path) = &self.path {
            //
            if let Ok(message) = git(path, &["log", "-1", "--pretty=%B", hash]) {
                //
                if let Some(captures) = message.find('#') {
                    let after_hash = &message[captures + 1..];
                    let pr_number: String = after_hash.chars().take_while(char::is_ascii_digit).collect();

                    if !pr_number.is_empty() {
                        return Some(pr_number);
                    }
                }
            }
        }
        None
    }

    pub fn current_branch(&self) -> String {
        self.path
            .as_ref()
            .and_then(|path| git(path, &["symbolic-ref", "--short", "-q", "HEAD"]).ok())
            .unwrap_or_else(|| "main".to_string())
    }

    fn remote(path: impl AsRef<Path>, remote: &str) -> Result<String, RepositoryError> {
        match git(&path, &["remote", "get-url", "--", remote]) {
            Ok(url) => Ok(url),
            Err(RepositoryError::CommandFailed(r)) if r.code() == Some(2) => {
                // An initialized Git repository without a remote.
                Err(RepositoryError::NoSuchRemote(remote.to_string()))
            }
            Err(e) => Err(e),
        }
    }

    pub fn url(current_dir: &str, paths: &[String]) -> Result<String, RepositoryError> {
        let is_git = is_git_repo(current_dir);

        let join_paths = || match paths.join(" ") {
            path if path == "." => current_dir.to_string(),
            path => path,
        };

        if !is_git {
            return Ok(join_paths());
        }

        let r = Self::from_path(current_dir)?;

        if paths.is_empty() {
            return Ok(r.http_url());
        }

        if paths.len() == 1 {
            let arg = &paths[0];
            let is_commit = is_valid_commit_hash(arg);

            if is_commit || is_pr_number(arg) {
                return if is_commit {
                    match r.pr_for_commit(arg) {
                        Some(pr_number) => Ok(r.pr_url(&pr_number)),
                        None => Ok(r.commit_url(arg)),
                    }
                } else {
                    Ok(r.pr_url(arg))
                };
            }
        }

        Ok(join_paths())
    }
}

impl fmt::Debug for GitRepository {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.http_url())
    }
}

impl fmt::Display for GitRepository {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.http_url())
    }
}

fn git(path: impl AsRef<Path>, args: &[&str]) -> Result<String, RepositoryError> {
    let out = Command::new("git")
        .args(args)
        .current_dir(path)
        .stderr(Stdio::null())
        .output()
        .map_err(RepositoryError::CouldNotExecute)?;
    if out.status.success() {
        Ok(str::from_utf8(&out.stdout)?.trim().to_string())
    } else {
        Err(RepositoryError::CommandFailed(out.status))
    }
}

pub fn is_git_repo(path: impl AsRef<Path>) -> bool {
    git(&path, &["rev-parse", "--git-dir"]).is_ok()
}

pub fn is_valid_commit_hash(hash: &str) -> bool {
    hash.len() >= 7 && hash.len() <= 40 && hash.chars().all(|c| c.is_ascii_hexdigit())
}

pub fn is_pr_number(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use testresult::TestResult;

    use super::*;

    #[test]
    fn test_git_repository_from_url() -> TestResult {
        // Test various URL formats with different hosts
        let repo = GitRepository::from_url("https://gitlab.com/org/repo")?;
        assert_eq!(repo.org, "org");
        assert_eq!(repo.name, "repo");
        assert_eq!(repo.http_url(), "https://gitlab.com/org/repo");

        let repo = GitRepository::from_url("git@bitbucket.org:org/repo.git")?;
        assert_eq!(repo.org, "org");
        assert_eq!(repo.name, "repo");
        assert_eq!(repo.http_url(), "https://bitbucket.org/org/repo");

        let repo = GitRepository::from_url("ssh://git@git.example.com/org/repo")?;
        assert_eq!(repo.org, "org");
        assert_eq!(repo.name, "repo");
        assert_eq!(repo.http_url(), "https://git.example.com/org/repo");

        Ok(())
    }

    #[test]
    fn test_git_repository_invalid() {
        assert!(GitRepository::from_url("invalid-url").is_err());
    }
}
