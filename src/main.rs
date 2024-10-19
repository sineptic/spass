#![warn(clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]
#![deny(clippy::missing_panics_doc)]
#![allow(clippy::redundant_closure_for_method_calls)]

use arboard::Clipboard;
use args::{Args, Command};
use clap::Parser;
use std::{
    fmt::Display,
    io::{stderr, stdin, stdout, Write},
    path::{Path, PathBuf},
    process::exit,
    ptr::drop_in_place,
    string::FromUtf8Error,
    sync::{Arc, LazyLock, Mutex},
    thread::sleep,
    time::Duration,
};
use thiserror::Error;

// TODO: load it smart
static CLIP_TIME: usize = 45;
static EDITOR_NAME: LazyLock<String> =
    LazyLock::new(|| std::env::var("EDITOR").unwrap_or("vi".to_string()));
static DEFAULT_GENERATED_LENGTH: usize = 25;

#[allow(clippy::option_option)]
mod args;
mod utils;

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    IO(#[from] std::io::Error),
    #[error(transparent)]
    GPG(#[from] gpgme::Error),
    #[error(transparent)]
    FromUtf(#[from] FromUtf8Error),
    #[error(transparent)]
    Clipboard(#[from] arboard::Error),

    #[error(
        "You must run:\n    {} init ...\n before you may use th password store",
        utils::how_i_invoked()
    )]
    PasswordStoreUninitialized,
    #[error("There is no password to put on the clipboard at line {line_number}")]
    NoPasswordAtLine { line_number: usize },
    #[error("Tree command not found. Try install one of [{}]", supported_commands.join(", "))]
    TreeCommandNotFound { supported_commands: Vec<String> },
    #[error("the entered passwords do not match")]
    PasswordsDontMatch,
    #[error("{pass_name} is not in the password store.\nNote: Your pass will have path {:?}", api::PASS_DIR_ROOT.join(pass_name).to_str().unwrap().to_string() + ".gpg")]
    PassDoesNotExist { pass_name: String },
}
type Result<T> = std::result::Result<T, Error>;

mod api;
pub use api::*;

#[allow(clippy::too_many_lines)]
fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    // dbg!(&args);
    match args.command {
        Command::Init { subfolder, gpg_ids } => api::init(subfolder, gpg_ids),
        Command::List { subfolder } => {
            check_uninitialized_store()?;
            let output = run_tree_cmd(&api::PASS_DIR_ROOT.join(subfolder))?;
            if output.status.success() {
                let stdout = String::from_utf8(output.stdout)?;
                println!("Password Store");
                for line in stdout.lines().skip(1) {
                    println!("{}", line.replace(".gpg", ""));
                }
            }
        }
        Command::Find { pass_names } => {
            check_uninitialized_store()?;
            println!("Search Terms: {}", pass_names.join(","));
            display_matches(pass_names)?;
        }
        Command::Show {
            pass_name,
            copy_line,
        } => {
            check_uninitialized_store()?;
            let copy_line = copy_line.map(|x| x.unwrap_or(1));
            let pass = std::io::read_to_string(
                // Safety: we drop `PassFile` after read all content
                unsafe { api::PassFile::open(pass_name.clone()) }?.content_reader()?,
            )?;
            if let Some(line_number) = copy_line {
                if line_number == 0 {
                    eprintln!("line numbers start from 1, but you write 0");
                    std::process::exit(1);
                }
                let content = pass
                    .lines()
                    .nth(line_number - 1)
                    .ok_or(Error::NoPasswordAtLine { line_number })?;
                clipboard_copy(content, &pass_name)?;
            } else {
                print!("{pass}");
            }
        }
        Command::Grep {
            search_string: _,
            grep_options: _,
        } => todo!("use grep command"),
        Command::Insert {
            echo,
            multiline,
            force,
            pass_name,
        } => {
            check_uninitialized_store()?;
            // Safety: we don't call to functions that may exit and don't drop anything
            let mut pass_file = unsafe { api::PassFile::create(pass_name.clone(), force) }?;
            let password = get_password_from_user(&pass_name, echo, multiline)?;
            pass_file.content_writer()?.write_all(password.as_bytes())?;
        }
        Command::Edit { pass_name } => {
            check_uninitialized_store()?;
            // Safety: we don't call to functions that may exit and don't drop anything
            let mut pass_file = unsafe { api::PassFile::open(pass_name) }?;
            let temp_path = pass_file.get_path_to_unencrypted();
            std::process::Command::new(&*EDITOR_NAME)
                .arg(temp_path)
                .spawn()?
                .wait()?;
        }
        Command::Generate {
            length,
            no_symbols,
            pass_name,
            in_place,
            force,
            clip,
        } => {
            check_uninitialized_store()?;
            let password = generate_password(length, no_symbols);

            if in_place {
                // Safety: dropped in end of if block
                let mut pass_file = unsafe { api::PassFile::open(pass_name.clone()) }?;
                let old_content = std::io::read_to_string(pass_file.content_reader()?)?;
                let old_content_tail = remove_first_line(&old_content);
                let new_content = password.clone() + "\n" + &old_content_tail;
                pass_file
                    .content_writer()?
                    .write_all(new_content.as_bytes())?;
            } else {
                // Safety: dropped in end of else block
                let mut pass_file = unsafe { api::PassFile::create(pass_name.clone(), force) }?;
                pass_file
                    .content_writer()?
                    .write_all((password.clone() + "\n").as_bytes())?;
            }
            if clip {
                clipboard_copy(&password, &pass_name)?;
            }
        }
        Command::Remove {
            pass_name,
            force,
            recursive,
        } => {
            check_uninitialized_store()?;
            let path = if recursive {
                api::PASS_DIR_ROOT.join(&pass_name)
            } else {
                api::get_pass_path(&pass_name)
            };
            if !path.exists() {
                return Err(Error::PassDoesNotExist {
                    pass_name: pass_name.clone(),
                }
                .into());
            }
            assert_ne!(path, *api::PASS_DIR_ROOT);

            // to make it lazy
            let agreement = || -> Result<bool> {
                print!("Are you sure you would like to delete {pass_name}? ");
                Ok(utils::yesno(false)?)
            };
            if force || agreement()? {
                if recursive {
                    std::fs::remove_dir_all(path)?;
                } else {
                    std::fs::remove_file(path)?;
                }
            }

            eprintln!("WARNING: Current version can not add this change to git");
        }
        Command::Rename {
            force,
            old_path: old_pass,
            new_path: new_pass,
        } => {
            check_uninitialized_store()?;
            let (old_root, recursive, new_root) = find_recursion(&old_pass, &new_pass)?;
            copy_move(
                CopyMove::Move,
                recursive,
                old_root,
                &new_root,
                force,
                old_pass,
                new_pass,
            )?;
        }
        Command::Copy {
            force,
            old_path: old_pass,
            new_path: new_pass,
        } => {
            check_uninitialized_store()?;
            let (old_root, recursive, new_root) = find_recursion(&old_pass, &new_pass)?;
            copy_move(
                CopyMove::Copy,
                recursive,
                old_root,
                &new_root,
                force,
                old_pass,
                new_pass,
            )?;
        }
        Command::Git {
            git_command_args: _,
        } => todo!(),
    };
    Ok(())
}

fn find_recursion(
    old_pass: &String,
    new_pass: &String,
) -> Result<(std::path::PathBuf, bool, std::path::PathBuf)> {
    let old_path_dir = api::PASS_DIR_ROOT.join(old_pass);
    let (old_root, recursive) = if old_path_dir.is_dir() {
        (old_path_dir, true)
    } else {
        let old_path_file = api::get_pass_path(old_pass);
        if !old_path_file.exists() {
            return Err(Error::PassDoesNotExist {
                pass_name: old_pass.clone(),
            });
        }
        (old_path_file, false)
    };
    let new_root = if recursive {
        api::PASS_DIR_ROOT.join(new_pass)
    } else {
        api::get_pass_path(new_pass)
    };
    Ok((old_root, recursive, new_root))
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum CopyMove {
    Copy,
    Move,
}
fn copy_move(
    copy_move: CopyMove,
    recursive: bool,
    old_root: PathBuf,
    new_root: &Path,
    force: bool,
    old_pass: String,
    new_pass: String,
) -> Result<()> {
    if recursive {
        assert_ne!(old_root, *api::PASS_DIR_ROOT);
        let mut pass_files = get_pass_files_recursively(&old_root, new_root)?;
        match copy_move {
            CopyMove::Copy => {
                for (pass_file, new_name) in &mut pass_files {
                    pass_file.copy(new_name.to_owned(), force)?;
                }
            }
            CopyMove::Move => {
                for (pass_file, new_name) in &mut pass_files {
                    pass_file.rename(new_name.to_owned(), force)?;
                }
                // NOTE: required to run before drop
                std::fs::remove_dir_all(old_root)?;
            }
        }
        drop(pass_files);
    } else {
        // Safety: pass file will be dropped at the and of else block.
        let mut pass_file = unsafe { api::PassFile::open(old_pass) }?;
        match copy_move {
            CopyMove::Copy => {
                pass_file.copy(new_pass, force)?;
            }
            CopyMove::Move => {
                pass_file.rename(new_pass, force)?;
            }
        }
        drop(pass_file);
    }
    Ok(())
}

/// # Returns
/// Vec<(pass file, new pass name if `old_root` change to `new_root`)>
fn get_pass_files_recursively(old_root: &Path, new_root: &Path) -> Result<Vec<(PassFile, String)>> {
    let files = walkdir::WalkDir::new(old_root).contents_first(true);
    let pass_files = files
        .into_iter()
        .filter_entry(|x| {
            x.file_type().is_file() && x.path().extension().is_some_and(|ext| ext == "gpg")
        })
        .filter_map(|x| x.ok())
        .map(|x| x.into_path())
        .map(|x| {
            (
                x.strip_prefix(&*api::PASS_DIR_ROOT)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .strip_suffix(".gpg")
                    .unwrap()
                    .to_string(),
                new_root
                    .join(x.strip_prefix(old_root).unwrap().to_str().unwrap())
                    .strip_prefix(&*api::PASS_DIR_ROOT)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .strip_suffix(".gpg")
                    .unwrap()
                    .to_string(),
            )
        })
        .map(|(old_pass_name, new_pass_name)| -> Result<_> {
            // Safety: pass files will be dropped at the and of else block.
            Ok((
                unsafe { api::PassFile::open(old_pass_name) }?,
                new_pass_name,
            ))
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(pass_files)
}

fn remove_first_line(old_content: &str) -> String {
    let mut old_content_tail = old_content.lines().skip(1).collect::<Vec<&str>>();
    if !old_content_tail.is_empty() {
        old_content_tail.push(""); // for "\n" at the and
    }

    old_content_tail.join("\n")
}

fn generate_password(length: usize, no_symbols: bool) -> String {
    use rand::prelude::*;
    let letters = String::from("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ");
    let numbers = String::from("0123456789");
    let symbols = String::from(r#"!\#"$%&'()*+,-./:;<=>?@[\\]^_`{¦}~"#);
    let chars = if no_symbols {
        letters.chars().chain(numbers.chars()).collect::<Vec<_>>()
    } else {
        letters
            .chars()
            .chain(numbers.chars())
            .chain(symbols.chars())
            .collect::<Vec<_>>()
    };
    let a = rand::distributions::Slice::new(&chars).unwrap();
    let mut b = rand::rngs::StdRng::sample_iter(rand::rngs::StdRng::from_entropy(), a);

    // FIXME: rewrite
    let mut password = String::new();
    for _ in 0..length {
        password.push(*b.next().unwrap());
    }
    password
}

fn get_password_from_user(pass_name: &str, echo: bool, multiline: bool) -> Result<String> {
    let password = if echo {
        print!("Enter password for {pass_name}: ");
        stdout().flush()?;
        let mut password = String::new();
        stdin().read_line(&mut password)?;
        password
    } else if multiline {
        todo!()
    } else {
        let password = rpassword::prompt_password(format!("Enter password for {pass_name}: "))?;
        if password != rpassword::prompt_password(format!("Retype password for {pass_name}: "))? {
            return Err(Error::PasswordsDontMatch);
        }
        password
    };
    Ok(password)
}

fn display_matches(pass_names: Vec<String>) -> Result<()> {
    let output = run_tree_cmd(&api::PASS_DIR_ROOT)?;
    if output.status.success() {
        let output = String::from_utf8(output.stdout)?;

        let lines = output.lines().skip(1);
        let matches = filter_matches(lines, pass_names)
            .map(|str| str.replace(".gpg", ""))
            .collect::<Vec<_>>()
            .join("\n");

        println!("{matches}");
    }

    Ok(())
}

fn filter_matches<'a>(
    lines: impl Iterator<Item = &'a str>,
    words: impl IntoIterator<Item = impl AsRef<[u8]>>,
) -> impl Iterator<Item = &'a str> {
    let finder = aho_corasick::AhoCorasick::builder()
        .ascii_case_insensitive(true)
        .build(words)
        .unwrap();

    lines.filter(move |str| finder.find(str).is_some())
}

fn run_tree_cmd(path: &Path) -> Result<std::process::Output> {
    let mut cmd_id = 0;
    let output = std::process::Command::new("eza")
        .arg(path)
        .args(["--tree", "--color=always", "--dereference"])
        .stderr(stderr())
        .output()
        .or_else(|_| {
            cmd_id = 1;
            std::process::Command::new("tree")
                .arg(path)
                .arg("-C")
                .arg("--noreport")
                .arg("--prune")
                .stderr(stderr())
                .output()
        })
        .map_err(|_| Error::TreeCommandNotFound {
            supported_commands: vec!["eza".to_string(), "tree".to_string()],
        })?;
    // Some genius tree developer print error to stdout
    if cmd_id == 1 {
        eprintln!("{}", String::from_utf8_lossy(&output.stdout));
    }
    Ok(output)
}

/// # Warning
/// On success this function doesn't return
fn clipboard_copy(content: &str, name: &str) -> anyhow::Result<()> {
    let clipboard = Arc::new(Mutex::new(Clipboard::new()?));
    clipboard.lock().unwrap().set_text(content)?;
    println!("Copied {name} to clipboard.");
    let same_clipboard = clipboard.clone();
    match ctrlc::set_handler(move || {
        clear_and_exit(&same_clipboard);
    }) {
        Ok(()) => {
            println!("Clipboard will be cleared in {CLIP_TIME} seconds or if you cancel program.");
        }
        Err(_) => {
            println!("Clipboard will be cleared in {CLIP_TIME} seconds, don't cancel program.");
        }
    };
    sleep(Duration::from_secs(CLIP_TIME as u64));
    clear_and_exit(&clipboard);
}

fn clear_and_exit(clipboard: &Mutex<Clipboard>) -> ! {
    match clipboard.lock() {
        Ok(mut clipboard) => {
            match clipboard.clear() {
                Ok(()) => {
                    // Safety: after drop we exit, so other can't get access to clipboard
                    unsafe {
                        drop_in_place(&mut *clipboard);
                    }
                    println!("Clipboard cleared.");
                }
                Err(err) => cant_clear_clipboard(err),
            };
        }
        Err(err) => cant_clear_clipboard(err),
    }
    exit(0);
}
fn cant_clear_clipboard(reason: impl Display) -> ! {
    eprintln!("Error: {reason}.");
    println!("Warning: clear clipboard manually.");
    exit(1);
}
