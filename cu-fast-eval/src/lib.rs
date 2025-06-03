use std::process::Command;
use std::env;

pub fn get_env_int(name: &str) -> Result<Option<usize>, String> {
    match env::var(name) {
        Ok(value) => {
            if !value.is_empty() {
                match value.parse::<usize>() {
                    Ok(value) => { Ok(Some(value)) }
                    Err(err) => { Err(format!("{} needs to be an integer. ({})", name, err)) }
                }
            }
            else {
                Ok(None)
            }
        }
        Err(err) => {
            match err {
                env::VarError::NotPresent => { Ok(None) }
                _ => { Err(format!("Unexpected error while handling {} environment variable. {}", name, err)) }
            }
        }
    }
}

// Prints (to stderr) a formatted useful version of the command used.
// NOTE: Multi-line arguments will be elided to their first line!
pub fn log_command(cmd: &Command) {
    let args: Vec<String> =
        cmd
            .get_args().map( |s| {
                let mut lines =
                    s.to_str().unwrap()
                    .lines()
                ;
                let first = lines.next().unwrap();
                match lines.next() {
                    // Only one line.
                    None => { first.to_string() }
                    // Multi-line
                    Some(_) => { format!("{}…", first) }
                }
            })
            .collect()
    ;
    eprintln!(" $ {}", shlex::try_join(args.iter().map(AsRef::as_ref).collect::<Vec<&str>>()).unwrap())
}
