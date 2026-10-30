use beyond_leaderless::{experiment, utility};
use std::{env, error::Error, fs, path::Path};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 4 || !matches!(args[1].as_str(), "run" | "utility") {
        return Err("usage: beyond-leaderless {run|utility} CONFIG OUTPUT".into());
    }
    let config: experiment::Config = serde_json::from_slice(&fs::read(&args[2])?)?;
    let output = Path::new(&args[3]);
    if output.exists() {
        return Err(format!("output already exists: {}", output.display()).into());
    }
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    if args[1] == "utility" {
        utility::write(&config, output)?;
    } else {
        let result = experiment::run(config)?;
        let converged = result
            .profiles
            .iter()
            .filter(|p| p.search.converged)
            .count();
        eprintln!(
            "{:.1}s; {converged}/{} grid equilibria",
            result.elapsed_seconds,
            result.profiles.len()
        );
        fs::write(output, serde_json::to_vec_pretty(&result)?)?;
        if converged != result.profiles.len() {
            return Err("some searches did not converge; inspect the saved profiles".into());
        }
    }
    Ok(())
}
