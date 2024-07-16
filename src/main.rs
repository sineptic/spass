use clap::{Parser, Subcommand};
use std::path::PathBuf;

// TODO: load from config
static CLIP_TIME: usize = 45;
static EDITOR_NAME: &str = "vi";
static DEFAULT_GENERATED_LENGTH: usize = 25;

#[derive(Parser, Debug)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    #[command(
        about = "Initialize new password storage and use gpg-id for encryption. Selectively reencrypt existing passwords using new gpg-id."
    )]
    Init {
        #[arg(long, short)]
        path: Option<PathBuf>,
        gpg_id: String,
    },
    #[command(about = "List passwords.")]
    Ls { subfolder: Option<PathBuf> },
    #[command(about = "List passwords that match pass-names.")]
    Find { pass_names: String },
    #[command(
        about = format!("Show existing password.")
    )]
    Show {
        pass_name: String,
        #[arg(
            long,
            short,
            value_name = "line-number",
            help = "Put it on the clipboard and clear board after {CLIP_TIME} seconds."
        )]
        clip: Option<Option<usize>>,
    },
    #[command(about = "Search for password files containing search-string when decrypted.")]
    Grep {
        search_string: String,
        grep_options: Vec<String>,
    },
    #[command(about = "Insert new password.")]
    Insert {
        #[arg(
            long,
            short,
            help = "Echo the password back to the console during entry."
        )]
        echo: bool,
        #[arg(long, short, help = "Entry may be multiline.")]
        multiline: bool,
        #[arg(
            long,
            short,
            help = "Don't prompt before overwriting existing password."
        )]
        force: bool,
        pass_name: String,
    },
    #[command(about = format!("Insert a new password or edit an existing password using {EDITOR_NAME}."))]
    Edit { pass_name: String },
    #[command(about = "Generate a new password.")]
    Generate {
        #[arg(long, short)]
        no_symbols: bool,
        #[arg(long, short, help = format!("Put it on the clipboard and clear board after {CLIP_TIME} seconds."))]
        clip: bool,
        #[arg(
            long,
            short,
            help = "Replace only the first line of an existing file with a new password."
        )]
        in_place: bool,
        #[arg(
            long,
            short,
            help = "Don't prompt before overwriting existing password."
        )]
        force: bool,
        pass_name: String,
        #[arg(default_value = DEFAULT_GENERATED_LENGTH.to_string())]
        length: Option<usize>,
    },
    #[command(about = "Remove existing password")]
    Rm {
        #[arg(long, short, help = "Remove directory instead")]
        recursive: bool,
        #[arg(long, short)]
        force: bool,
        pass_name: String,
    },
    #[command(about = "Renames or moves old-path to new-path, selectively reencrypting.")]
    Mv {
        #[arg(long, short)]
        force: bool,
        old_path: PathBuf,
        new_path: PathBuf,
    },
    #[command(about = "Copies old-path to new-path, selectively reencrypting.")]
    Cp {
        #[arg(long, short)]
        force: bool,
        old_path: PathBuf,
        new_path: PathBuf,
    },
    #[command(
        about = "If the password store is a git repository, execute a git command specified by git-command-args."
    )]
    Git { git_command_args: Vec<String> },
}

fn main() {
    let cli = Cli::parse();
    dbg!(cli);
}
