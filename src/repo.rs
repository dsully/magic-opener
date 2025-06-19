use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::str;

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
            return Err(RepositoryError::Spec(url.to_string()));
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

#[cfg(test)]
mod tests {
    use super::*;
    use testresult::TestResult;

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
