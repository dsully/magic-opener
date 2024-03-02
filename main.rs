use std::env;
use std::io::{stdout, Write};
use std::net::TcpStream;
use std::process::{self, Command, Stdio};

use clap::Parser;
use parse_git_url::GitUrl;
use shellexpand::tilde;

const LOCALHOST: &str = "localhost";
const OPEN: &str = "/usr/bin/open";
const PORT: u16 = 2226;
const REMOTE_NAME: &str = "origin";

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None, disable_help_flag = true)]
#[allow(clippy::upper_case_acronyms)]
struct CLI {
    #[clap(short, long, help = "Print the URL to stdout instead of opening it.")]
    print: bool,

    #[clap(
        allow_hyphen_values = true,
        trailing_var_arg = true,
        required = false,
        help = "Path to a Git repository. Otherwise the current directory will be used."
    )]
    path: Vec<String>,
}

fn git_url() -> Option<String> {
    Command::new("git")
        .args(["remote", "get-url", REMOTE_NAME])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .ok()
        .and_then(|output| GitUrl::parse(String::from_utf8_lossy(&output.stdout).trim_end()).ok())
        .and_then(|parsed| {
            parsed
                .host
                .map(|host| format!("https://{}/{}", host, parsed.fullname))
        })
}

fn main() {
    let args = CLI::parse();

    let current_dir = env::current_dir()
        .expect("Failed to get current directory")
        .to_string_lossy()
        .to_string();

    let remote_path = if args.path.is_empty() {
        match git_url() {
            Some(url) => url,
            None => current_dir.to_owned(),
        }
    } else {
        match args.path.join(" ") {
            path if path == "." => current_dir.to_owned(),
            path => path,
        }
    };

    if remote_path.starts_with('-') {
        let command = if remote_path == "--help" {
            vec!["-h"]
        } else {
            args.path.iter().map(String::as_str).collect()
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
        remote_path.to_owned()
    } else if ssh_tty {
        //
        let client_home = env::var("SSH_CLIENT_HOME")
            .expect("No $SSH_CLIENT_HOME set! It must be set in the SSH client config.");

        let expanded_path = tilde(&remote_path);

        if expanded_path.starts_with("/bits") {
            format!("{}/Mounts{}", client_home, expanded_path)
        } else {
            expanded_path.into_owned()
        }
    } else {
        remote_path.to_owned()
    };

    if args.print {
        println!("{}", remote_path);
    } else if ssh_tty {
        let mut stream = TcpStream::connect((LOCALHOST, PORT))
            .expect("Unable to create a socket for localhost:2226");

        stream
            .write_all(remote_path.as_bytes())
            .expect("Couldn't write remote path to socket.");
    } else {
        let mut args = vec![remote_path.as_str()];

        if remote_path.contains("://") {
            args.insert(0, "--background");
        }

        Command::new(OPEN)
            .args(&args)
            .spawn()
            .expect("Failed to open URL");
    }
}
