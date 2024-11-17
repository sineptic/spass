use super::{Error, Result};
use crate::utils::{self, yesno};
use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::LazyLock,
};

pub static PASS_DIR_ROOT: LazyLock<PathBuf> = LazyLock::new(|| {
    PathBuf::from(std::env::var("HOME").expect("env variable `HOME` should be set"))
        .join(".password-store")
});

#[derive(Debug)]
#[must_use]
pub struct PassFile {
    pass_name: String,
    temp_path: tempfile::TempPath,
    /// Changes should be added to git.
    maybe_modified: bool,
    commit_msg: Option<String>,
}
impl PassFile {
    /// # Safety
    /// You must drop `EncryptedFile`.
    pub unsafe fn open(pass_name: String) -> Result<Self> {
        check_uninitialized_store()?;
        let content = crate::utils::read_to_vec(get_readonly_pass_file(pass_name.clone())?)?;
        let mut gpg = gpgme::Context::from_protocol(gpgme::Protocol::OpenPgp)?;
        let content = {
            let mut buf = Vec::new();
            gpg.decrypt(&content, &mut buf)?;
            buf
        };
        PassFile::new(pass_name, &content)
    }
    /// # Safety
    /// You must drop `EncryptedFile`.
    #[allow(clippy::missing_panics_doc/* Reason: get_root() is not filesystem root */)]
    pub unsafe fn create(pass_name: String, force: bool) -> Result<Self> {
        check_uninitialized_store()?;
        let path = PASS_DIR_ROOT.join(pass_name.clone() + ".gpg");
        std::fs::create_dir_all(path.parent().unwrap())?;
        if force {
            File::create(&path)?;
        } else {
            File::create_new(&path).or_else(|err| {
                if err.kind() == std::io::ErrorKind::AlreadyExists {
                    print!("An entry already exists for {path:?}. Overwrite it? ");
                    if utils::yesno(false)? {
                        Ok(File::create(&path)?)
                    } else {
                        Err(err)
                    }
                } else {
                    Err(err)
                }
            })?;
        };
        let mut temp = PassFile::new(pass_name, &[])?;
        temp.maybe_modified = true;
        Ok(temp)
    }
    /// # Warning
    /// `modified` by default set to true.
    fn new(pass_name: String, content: &[u8]) -> Result<Self> {
        let mut temp_file = utils::create_temp_file()?;
        temp_file.write_all(content)?;
        Ok(Self {
            pass_name,
            temp_path: temp_file.into_temp_path(),
            maybe_modified: false,
            commit_msg: None,
        })
    }
    /// # Warning
    /// - You can see all changes only after `flush()` or `drop()`.
    /// - Current changes don't affect old pass file.
    /// # Note
    /// If function return error, [`PathFile`] stay unchanged.
    pub fn copy(&mut self, new_name: String, force: bool) -> std::io::Result<()> {
        let new_path = get_pass_path(&new_name);
        let user_agreement = || -> std::io::Result<bool> {
            print!("An entry already exists for {new_path:?}. Overwrite it? ");
            yesno(false)
        };
        if force || !new_path.exists() || user_agreement()? {
            self.pass_name = new_name;
            self.maybe_modified = true;
        }
        Ok(())
    }
    /// # Warning
    /// You must specify commit message before running this method.
    /// # Note
    /// If function return error, `PathFile` stay unchanged.
    pub fn rename(&mut self, new_name: String, force: bool) -> Result<()> {
        let new_path = get_pass_path(&new_name);
        let user_agreement = || -> std::io::Result<bool> {
            print!("An entry already exists for {new_path:?}. Overwrite it? ");
            yesno(false)
        };
        if force || !new_path.exists() || user_agreement()? {
            let prev_pass_name = self.pass_name.clone();
            self.pass_name = new_name;
            self.maybe_modified = true;
            self.flush()?;
            std::fs::remove_file(get_pass_path(&prev_pass_name))?;
        }
        Ok(())
    }
    #[must_use]
    pub fn get_path_to_unencrypted(&mut self) -> &Path {
        self.maybe_modified = true;
        &self.temp_path
    }
    pub fn content_writer(&mut self) -> std::io::Result<impl Write + '_> {
        self.maybe_modified = true;
        File::create(&self.temp_path)
    }
    pub fn content_reader(&self) -> std::io::Result<impl Read + '_> {
        File::open(&self.temp_path)
    }
    pub fn set_commit_msg(&mut self, msg: String) {
        self.commit_msg = Some(msg);
    }
    /// Write all content from temp file to encrypted file.
    #[allow(clippy::missing_panics_doc/* Reason: get_pass_path() is not filesystem root */)]
    pub fn flush(&mut self) -> Result<()> {
        if self.maybe_modified {
            let final_version = crate::utils::read_to_vec(File::open(&*self.temp_path)?)?;
            let path = get_pass_path(&self.pass_name);

            std::fs::create_dir_all(path.parent().unwrap())?;
            let mut pass_file = File::create(path)?;
            let encrypted = encrypt(&self.pass_name, &final_version)?;
            pass_file.write_all(&encrypted)?;

            if let Some(ref commit_msg) = self.commit_msg {
                let result = crate::git::commit_file(
                    PASS_DIR_ROOT.as_os_str(),
                    &(self.pass_name.clone() + ".gpg"),
                    commit_msg,
                );
                if let Err(err) = result {
                    match err {
                        Error::PassStoreShouldBeGitRepo => (),
                        err => return Err(err),
                    }
                }
            } else {
                eprintln!("NOTE: not added to git because `commit_msg` not specified.");
            }
            self.maybe_modified = false;
        }
        Ok(())
    }
}

impl Drop for PassFile {
    fn drop(&mut self) {
        let error_msg = format!(
            "Can't delete temp file with path {:?} and then encrypt it's content to {:?}. Save and delete it manually",
            &*self.temp_path, self.pass_name
        );
        self.flush().expect(&error_msg);
    }
}

pub fn init(subfolder: String, recipients: Vec<String>) {
    dbg!(subfolder, recipients);
    todo!()
}

fn get_readonly_pass_file(pass_name: String) -> Result<File> {
    check_uninitialized_store()?;
    let path = get_pass_path(&pass_name);

    File::open(&path).map_err(|err| {
        let err = err.into();
        if let Error::IO(err) = err {
            if err.kind() == std::io::ErrorKind::NotFound {
                Error::PassDoesNotExist { pass_name }
            } else {
                err.into()
            }
        } else {
            err
        }
    })
}
pub fn get_pass_path(pass_name: &impl ToString) -> PathBuf {
    PASS_DIR_ROOT.join(pass_name.to_string() + ".gpg")
}

fn get_recipients(pass_name: &str) -> Result<Vec<String>> {
    assert!(!pass_name.is_empty());
    let mut path: PathBuf = PASS_DIR_ROOT.join(pass_name);
    loop {
        path = path.parent().unwrap().to_owned();
        if let Ok(mut file) = File::open(path.join(".gpg-id")) {
            let mut recipients = String::new();
            file.read_to_string(&mut recipients)?;
            let recipients = recipients.lines().map(|x| x.to_owned());
            break Ok(recipients.collect::<Vec<_>>());
        }

        if path == *PASS_DIR_ROOT {
            break Err(Error::PasswordStoreUninitialized);
        }
    }
}

fn encrypt(pass_name: &str, content: &[u8]) -> Result<Vec<u8>> {
    let mut gpg = gpgme::Context::from_protocol(gpgme::Protocol::OpenPgp)?;
    let recipients = get_recipients(pass_name)?;
    let recipients = gpg
        .find_keys(recipients)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let content = {
        let mut buf = Vec::new();
        gpg.encrypt(&recipients, content, &mut buf)?;
        buf
    };
    Ok(content)
}

pub fn check_uninitialized_store() -> Result<()> {
    if let Ok(Some(_)) = PASS_DIR_ROOT.read_dir().map(|mut x| x.next()) {
        Ok(())
    } else {
        Err(Error::PasswordStoreUninitialized)
    }
}
