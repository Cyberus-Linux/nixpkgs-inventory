use std::env;

pub mod evaluation;
pub mod nix;

pub fn get_env_int(name: &str) -> Result<Option<usize>, String> {
    match env::var(name) {
        Ok(value) => {
            if !value.is_empty() {
                match value.parse::<usize>() {
                    Ok(value) => Ok(Some(value)),
                    Err(err) => Err(format!("{} needs to be an integer. ({})", name, err)),
                }
            } else {
                Ok(None)
            }
        }
        Err(err) => match err {
            env::VarError::NotPresent => Ok(None),
            _ => Err(format!(
                "Unexpected error while handling {} environment variable. {}",
                name, err
            )),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    mod lib {
        use super::*;

        #[test]
        fn test_get_env_int() {
            unsafe { env::remove_var("TEST_VAR") };
            assert_eq!(get_env_int("TEST_VAR"), Ok(None));

            unsafe { env::set_var("TEST_VAR", "0") };
            assert_eq!(get_env_int("TEST_VAR"), Ok(Some(0)));

            unsafe { env::set_var("TEST_VAR", "42") };
            assert_eq!(get_env_int("TEST_VAR"), Ok(Some(42)));

            unsafe { env::set_var("TEST_VAR", "bogus") };
            // Let's make sure the message is the one Rust gives us.
            let parsing_error = "bogus".parse::<usize>().err().unwrap().to_string();
            assert_eq!(
                get_env_int("TEST_VAR"),
                Err(format!(
                    "{} needs to be an integer. ({})",
                    "TEST_VAR", parsing_error
                ))
            );
        }
    }
}
