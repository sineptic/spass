use std::io::Write;

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

#[derive(PartialEq)]
pub enum YesNo {
    Yes,
    No,
}
pub fn yesno(safer: YesNo) -> std::io::Result<YesNo> {
    match safer {
        YesNo::Yes => print!("[Y/n] "),
        YesNo::No => print!("[y/N] "),
    };
    std::io::stdout().flush()?;
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    match answer.to_lowercase().trim() {
        "y" | "yes" => Ok(YesNo::Yes),
        "n" | "no" => Ok(YesNo::No),
        "" => Ok(safer),
        _ => panic!("User can't write 1 letter!"),
    }
}
