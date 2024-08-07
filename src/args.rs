use crate::{CLIP_TIME, DEFAULT_GENERATED_LENGTH, EDITOR_NAME};
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(version)]
pub(crate) struct Args {
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Subcommand, Debug)]
pub(crate) enum Command {
    #[command(
        about = "Initialize new password storage and use gpg-id for encryption. Selectively reencrypt existing passwords using new gpg-id."
    )]
    Init {
        #[arg(
            long = "path",
            short = 'p',
            value_name = "subfolder",
            default_value = ""
        )]
        subfolder: String,
        #[arg(required(true))]
        gpg_ids: Vec<String>,
    },
    #[command(visible_alias = "ls", about = "List passwords.")]
    List {
        #[arg(default_value = "")]
        subfolder: String,
    },
    #[command(
        visible_alias = "search",
        about = "List passwords that match pass-names."
    )]
    Find {
        #[arg(required(true))]
        pass_names: Vec<String>,
    },
    #[command(about = "Show existing password.")]
    Show {
        pass_name: String,
        #[arg(
            long = "clip",
            short = 'c',
            value_name = "line-number",
            help = format!("Put it on the clipboard and clear board after {CLIP_TIME} seconds."),
        )]
        copy_line: Option<Option<usize>>,
        // TODO: QRCode
        //
        // #[arg(
        //     long("qrcode"),
        //     short('q'),
        //     value_name = "line-number",
        //     help = format!("Put it on the clipboard and clear board after {CLIP_TIME} seconds."),
        // )]
        // qrcode_line: Option<Option<usize>>,
    },
    #[command(about = "Search for password files containing search-string when decrypted.")]
    Grep {
        search_string: String,
        grep_options: Vec<String>,
    },
    #[command(visible_alias = "add", about = "Insert new password.")]
    Insert {
        #[arg(
            conflicts_with("multiline"),
            long,
            short,
            help = "Echo the password back to the console during entry."
        )]
        echo: bool,
        #[arg(conflicts_with("echo"), long, short, help = "Entry may be multiline.")]
        multiline: bool,
        #[arg(
            long,
            short,
            help = "Don't prompt before overwriting existing password."
        )]
        force: bool,
        pass_name: String,
    },
    #[command(about = format!("Insert a new password or edit an existing password using {}.", *EDITOR_NAME))]
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
            conflicts_with = "force",
            help = "Replace only the first line of an existing file with a new password."
        )]
        in_place: bool,
        #[arg(
            long,
            short,
            conflicts_with = "in_place",
            help = "Don't prompt before overwriting existing password."
        )]
        force: bool,
        pass_name: String,
        #[arg(long, short, default_value = DEFAULT_GENERATED_LENGTH.to_string())]
        length: usize,
    },
    #[command(visible_aliases = ["rm", "delete"], about = "Remove existing password")]
    Remove {
        #[arg(long, short, help = "Remove directory instead")]
        recursive: bool,
        #[arg(long, short)]
        force: bool,
        pass_name: String,
    },
    #[command(
        visible_alias = "mv",
        about = "Renames or moves old-path to new-path, selectively reencrypting."
    )]
    Rename {
        #[arg(long, short)]
        force: bool,
        old_path: String,
        new_path: String,
    },
    #[command(
        visible_alias = "cp",
        about = "Copies old-path to new-path, selectively reencrypting."
    )]
    Copy {
        #[arg(long, short)]
        force: bool,
        old_path: String,
        new_path: String,
    },
    #[command(
        about = "If the password store is a git repository, execute a git command specified by git-command-args."
    )]
    Git { git_command_args: Vec<String> },
}
