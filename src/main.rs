use std::collections::HashMap;
use std::process::ExitCode;

use quickcall::messages::error;
use quickcall::run;

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(cause) => {
            eprintln!("{}", error(&format!("cannot read cwd: {cause}")));
            return ExitCode::from(1);
        }
    };
    let executable = match std::env::current_exe() {
        Ok(path) => path,
        Err(cause) => {
            eprintln!("{}", error(&format!("cannot resolve executable: {cause}")));
            return ExitCode::from(1);
        }
    };
    let env: HashMap<String, String> = std::env::vars().collect();

    match run(&argv, &cwd, &env, &executable) {
        Ok(output) => {
            print!("{}", output.stdout);
            eprint!("{}", output.stderr);
            ExitCode::from(u8::try_from(output.exit).unwrap_or(1))
        }
        Err(cause) => {
            eprintln!("{}", error(&cause.message));
            ExitCode::from(u8::try_from(cause.exit_code).unwrap_or(1))
        }
    }
}
