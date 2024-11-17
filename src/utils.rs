use crate::Result;
use std::{io::Write, path::PathBuf};

pub fn read_to_vec(mut source: impl std::io::Read) -> std::io::Result<Vec<u8>> {
    let mut buf = Vec::new();
    source.read_to_end(&mut buf)?;
    Ok(buf)
}

pub fn how_i_invoked() -> String {
    std::path::PathBuf::from(std::env::args().next().unwrap())
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned()
}

/// true it's yes
/// false it's no
#[allow(clippy::match_bool)]
pub fn yesno(safer: bool) -> std::io::Result<bool> {
    match safer {
        true => print!("[Y/n] "),
        false => print!("[y/N] "),
    };
    std::io::stdout().flush()?;
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    match answer.to_lowercase().trim() {
        "y" | "yes" => Ok(true),
        "n" | "no" => Ok(false),
        "" => Ok(safer),
        _ => panic!("User can't write 1 letter!"),
    }
}

pub fn create_temp_file() -> Result<tempfile::NamedTempFile> {
    let template = format!("{}.XXXXXXXXXXXXX", how_i_invoked());
    let secure_tempdir = PathBuf::from("/dev/shm/").join(&template);
    std::fs::create_dir_all(&secure_tempdir)?;
    let temp_file = tempfile::NamedTempFile::new_in(&secure_tempdir).or_else(|_| {
        #[rustfmt::skip]
                print!(
"Your system does not have /dev/shm, which means that it may
be difficult to entirely erase the temporary non-encrypted
password file after editing.

Are you sure you would like to continue? "
                );
        if yesno(false)? {
            std::process::exit(1);
        }
        tempfile::NamedTempFile::new()
    })?;
    Ok(temp_file)
}
