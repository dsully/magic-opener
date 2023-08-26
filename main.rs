use clap::Parser;
use git2::Repository;
use git_url_parse::GitUrl;
use std::{error::Error, process};

const REMOTE_NAME: &str = "origin";

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(short, long, help = "Print the URL to stdout instead of opening it.")]
    print: bool,

    #[clap(help = "Path to a Git repository. Otherwise the current directory will be used.")]
    path: Option<String>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Cli::parse();

    let path = match &args.path {
        Some(path) => path,
        _ => ".",
    };

    let repository = Repository::discover(path).unwrap_or_else(|_| {
        println!("Unable to open repository at path: {:?}", path);
        process::exit(1);
    });

    let remote = repository.find_remote(REMOTE_NAME).unwrap_or_else(|e| {
        println!("Could not retrieve remote: {}", e);
        process::exit(1);
    });

    let remote_url = remote.url().unwrap_or_else(|| {
        println!("Could not retrieve remote url.");
        process::exit(1);
    });

    let parsed = GitUrl::parse(remote_url).unwrap_or_else(|e| {
        println!("Could not parse the git remote url: {}", e);
        process::exit(1);
    });

    let url = match parsed.host {
        Some(host) => format!("https://{}/{}", host, parsed.fullname),
        None => {
            println!("Did not match any patterns for remote: {}", remote_url);
            process::exit(1);
        }
    };

    if args.print {
        println!("{}", url);
    } else {
        open::that(url)?
    }

    Ok(())
}
