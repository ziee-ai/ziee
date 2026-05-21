use clap::ArgMatches;

use super::util::repo_relative;

pub fn run(matches: &ArgMatches) -> i32 {
    let flavor = matches.get_one::<String>("flavor").map(String::as_str).unwrap_or("full");
    let output = matches.get_one::<String>("output");

    let script = repo_relative("src-app/sandbox-rootfs/build.sh");
    if !script.exists() {
        eprintln!("missing: {}", script.display());
        return 1;
    }
    let mut args: Vec<String> = vec!["--flavor".into(), flavor.to_string()];
    if let Some(o) = output {
        args.push("--output".into());
        args.push(o.clone());
    }
    std::process::Command::new(&script)
        .args(&args)
        .status()
        .map(|s| s.code().unwrap_or(1))
        .unwrap_or_else(|e| {
            eprintln!("failed to invoke {}: {e}", script.display());
            1
        })
}
