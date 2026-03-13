mod codegen;
mod colors;
mod commands;
mod fmt;
mod lexer;
mod parser;
#[cfg(test)]
mod tests;
mod typechecker;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let mut flags: Vec<&str> = Vec::new();
    let mut positional: Vec<&str> = Vec::new();
    for arg in &args[1..] {
        if arg.starts_with('-') {
            flags.push(arg.as_str());
        } else {
            positional.push(arg.as_str());
        }
    }

    let subcmd = positional.first().copied().unwrap_or("");

    let opt_level: Option<&str> = flags.iter().find_map(|f| {
        if *f == "--release" {
            Some("ReleaseSmall")
        } else if let Some(val) = f.strip_prefix("--release=") {
            match val {
                "safe" => Some("ReleaseSafe"),
                "fast" => Some("ReleaseFast"),
                "small" => Some("ReleaseSmall"),
                other => {
                    eprintln!(
                        "Unknown release mode: '{}'. Use safe, fast, or small.",
                        other
                    );
                    std::process::exit(1);
                }
            }
        } else {
            None
        }
    });

    match subcmd {
        "check" => {
            let fix = flags.contains(&"--fix");
            let path = positional.get(1).copied();
            commands::check::run(path, fix);
        }
        "build" => {
            let input = positional.get(1).copied().unwrap_or_else(|| {
                eprintln!("Usage: zyre build <file.zy>");
                std::process::exit(1);
            });
            let source = commands::read_file(input);
            let ast = commands::check::check_source(&source, input);
            commands::build::run(input, &ast, opt_level);
        }
        "run" => {
            let input = positional.get(1).copied().unwrap_or_else(|| {
                eprintln!("Usage: zyre run <file.zy>");
                std::process::exit(1);
            });
            let source = commands::read_file(input);
            let ast = commands::check::check_source(&source, input);
            commands::run::run(input, &ast, opt_level);
        }
        "clean" => commands::clean::run(),
        _ => {
            eprintln!(
                "Usage: zyre <run|build|check|clean> [file.zy] [--release[=safe|fast|small]]"
            );
            std::process::exit(1);
        }
    }
}
