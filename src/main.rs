#![deny(clippy::all, clippy::pedantic, clippy::unwrap_used)]
#![allow(
    clippy::module_name_repetitions,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,

    // Ignore clippy for the generated file from shadow-rs.
    // https://github.com/baoyachi/shadow-rs/issues/151
    clippy::non_ascii_literal,
    clippy::print_stdout,
    clippy::needless_raw_strings,
    clippy::needless_raw_string_hashes
)]

use std::env;
use std::io::{Write, stdout};
use std::net::TcpStream;
use std::os::unix::process::CommandExt;
use std::process::{self, Command, Stdio};

use clap::{Arg, ArgAction, Command as ClapCommand};

mod parser;
mod repo;

use repo::{GitRepository, is_git_repo};

const LOCALHOST: &str = "localhost";
const OPEN: &str = "/usr/bin/open";
const PORT: u16 = 2226;

fn expand_tilde(path: &str) -> String {
    //
    if path.starts_with("~/") || path == "~" {
        if let Ok(home) = env::var("HOME") {
            return path.replacen('~', &home, 1);
        }
    }

    path.to_string()
}

fn main() {
    let cli = ClapCommand::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .arg(
            Arg::new("print")
                .short('p')
                .long("print")
                .help("Print the URL to stdout instead of opening it")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("path")
                .help("Path to a Git repository (defaults to current directory)")
                .num_args(0..)
                .trailing_var_arg(true)
                .allow_hyphen_values(true)
                .value_name("PATH"),
        );

    let matches = cli.get_matches();

    let paths: Vec<String> = matches
        .get_many::<String>("path")
        .unwrap_or_default()
        .cloned()
        .collect();

    let current_dir = env::current_dir()
        .expect("Failed to get current directory")
        .to_string_lossy()
        .to_string();

    let remote_path = if paths.is_empty() && is_git_repo(&current_dir) {
        GitRepository::from_path(&current_dir)
            .expect("Could not open Git repository")
            .http_url()
    } else {
        match paths.join(" ") {
            path if path == "." => current_dir.clone(),
            path => path,
        }
    };

    if remote_path.starts_with('-') {
        let command = if remote_path == "--help" {
            vec!["-h".to_string()]
        } else {
            paths
        };

        let output = Command::new(OPEN)
            .args(command)
            .stderr(Stdio::inherit())
            .output()
            .expect("Failed to run command");

        stdout()
            .write_all(&output.stderr)
            .expect("Failed to write to stdout");

        process::exit(0);
    }

    let ssh_tty = env::var_os("SSH_TTY").is_some();

    let remote_path = if remote_path.contains("://") {
        remote_path.clone()
    } else if ssh_tty {
        //
        let client_home = env::var("SSH_CLIENT_HOME")
            .expect("No $SSH_CLIENT_HOME set! It must be set in the SSH client config.");

        let expanded_path = expand_tilde(&remote_path);

        if expanded_path.starts_with("/bits") {
            format!("{client_home}/Mounts{expanded_path}")
        } else {
            expanded_path
        }
    } else {
        remote_path.clone()
    };

    if matches.get_flag("print") {
        println!("{remote_path}");
        return;
    }

    if ssh_tty {
        let mut stream = TcpStream::connect((LOCALHOST, PORT))
            .expect("Unable to create a socket for localhost:2226");

        stream
            .write_all(remote_path.as_bytes())
            .expect("Couldn't write remote path to socket.");

        return;
    }

    let mut args = vec![remote_path.as_str()];

    if remote_path.contains("://") {
        args.insert(0, "--background");
    }

    let _ = Command::new(OPEN).args(&args).exec();
}
