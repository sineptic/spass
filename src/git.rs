use std::{
    ffi::OsStr,
    io::Write,
    path::PathBuf,
    process::{Command, ExitStatus},
};

type Result<T> = std::result::Result<T, crate::Error>;

pub fn verify_git_initialized(path: &OsStr) -> Result<()> {
    let cmd = Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("rev-parse")
        .arg("--is-inside-work-tree")
        .output()?;

    if String::from_utf8_lossy(&cmd.stdout).trim() == "true" {
        Ok(())
    } else {
        Err(crate::Error::PassStoreShouldBeGitRepo)
    }
}
pub fn should_sign_commits(path: &OsStr) -> Result<bool> {
    let cmd = Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("config")
        .arg("bool")
        .args(["--get", "pass.signcommits"])
        .output()?;
    if String::from_utf8_lossy(&cmd.stdout).trim() == "true" {
        Ok(true)
    } else {
        Ok(false)
    }
}
pub fn command(path: &OsStr, args: impl IntoIterator<Item = String>) -> Result<ExitStatus> {
    verify_git_initialized(path)?;
    Ok(Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .spawn()?
        .wait()?)
}
pub fn init(path: &OsStr, other_args: impl IntoIterator<Item = String>) -> Result<()> {
    if verify_git_initialized(path).is_ok() {
        return Err(crate::Error::GitRepoAlreadyInitialized);
    }
    let cmd = Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("init")
        .args(other_args)
        .output()?;
    if !cmd.status.success() {
        eprintln!("{}", String::from_utf8_lossy(&cmd.stderr));
        return Err(crate::Error::CantInitGitRepo);
    }
    commit_all(path, "Add current contents of password store.")?;
    writeln!(
        std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(PathBuf::from(path).join(".gitattributes"))?,
        "*.gpg diff=gpg"
    )?;
    commit_file(
        path,
        ".gitattributes",
        "Configure git repository for gpg file diff.",
    )?;
    Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("config")
        .arg("--local")
        .args(["diff.gpg.binary", "true"])
        .output()?;
    Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("config")
        .arg("--local")
        .args([
            "diff.gpg.textconv",
            // TODO: add options from PASSWORD_STORE_GPG_OPTS
            "gpg -d --quiet --yes --compress-algo=none --no-encrypt-to",
        ])
        .output()?;
    Ok(())
}
pub fn unstage_all(path: &OsStr) -> Result<()> {
    Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("reset")
        .output()?;
    Ok(())
}
fn stage_file(path: &OsStr, file_name: &str) -> Result<()> {
    Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("add")
        .arg(file_name)
        .output()?;

    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("status")
        .args(["--porcelain", file_name])
        .output()?;
    if String::from_utf8_lossy(&output.stdout).is_empty() {
        dbg!(
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return Err(crate::Error::CantStageFile {
            file_name: file_name.to_owned(),
        });
    }
    Ok(())
}
fn stage_all(path: &OsStr) -> Result<()> {
    Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("add")
        .arg("--all")
        .output()?;
    Ok(())
}
fn commit(path: &OsStr, message: &str) -> Result<()> {
    verify_git_initialized(path)?;
    let mut binding = Command::new("git");
    let cmd = binding.arg("-C").arg(path).arg("commit");
    if should_sign_commits(path)? {
        cmd.arg("-S");
    }
    let cmd = cmd.args(["-m", message]).output()?;

    if cmd.status.success() {
        Ok(())
    } else {
        eprintln!("{}", String::from_utf8_lossy(&cmd.stderr));
        Err(crate::Error::CantCommit)
    }
}
pub fn commit_all(path: &OsStr, message: &str) -> Result<()> {
    verify_git_initialized(path)?;
    stage_all(path)?;
    commit(path, message)?;
    Ok(())
}
pub fn commit_file(path: &OsStr, file_name: &str, message: &str) -> Result<()> {
    verify_git_initialized(path)?;
    stage_file(path, file_name)?;
    commit(path, message)?;
    Ok(())
}
