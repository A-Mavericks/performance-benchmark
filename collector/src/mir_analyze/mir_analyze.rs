use std::{
    fs::read_dir,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use anyhow::bail;

use crate::{
    benchmark::{benchmark::Benchamrk, profile::Profiles},
    compile_time::discover_benchmark_suit,
    mir_analyze::analyze::{count::count_mir_entry, reader::read_mir},
    toolchain::LocalToolchain,
};

use super::mirs::mir::MIR;

/// Get all benchmark directories from `benchmark_dir` and
/// generate mir file for each benchmark. Then do analysis
/// on the generated mir file.
pub(crate) fn entry(
    ltc: LocalToolchain,
    benchmarks_dir: PathBuf,
    out_dir: PathBuf,
) -> anyhow::Result<()> {
    for benchmark in discover_benchmark_suit(&benchmarks_dir)? {
        let mirs = generate_mir(&benchmark, &ltc)?;

        do_analyze(mirs, &out_dir)?;
    }

    Ok(())
}

fn generate_mir(benchmark: &Benchamrk, ltc: &LocalToolchain) -> anyhow::Result<Vec<MIR>> {
    let tmp_dir = benchmark.make_temp_dir(&benchmark.path)?;

    let mut cmd = Command::new(Path::new(&ltc.cargo));
    cmd
        // Not all cargo invocations (e.g. `cargo clean`) need all of these
        // env vars set, but it doesn't hurt to have them.
        .env("RUSTC", &ltc.rustc)
        // We separately pass -Cincremental to the leaf crate --
        // CARGO_INCREMENTAL is cached separately for both the leaf crate
        // and any in-tree dependencies, and we don't want that; it wastes
        // time.
        .env("CARGO_INCREMENTAL", "0")
        // We need to use -Z flags (for example, to force enable ICH
        // verification) so unconditionally enable unstable features, even
        // on stable compilers.
        .env("RUSTC_BOOTSTRAP", "1")
        .current_dir(tmp_dir.path())
        .arg("rustc")
        .arg("--profile")
        .arg("release")
        .arg("--manifest-path")
        .arg(
            &benchmark
                .config
                .cargo_toml
                .clone()
                .unwrap_or_else(|| String::from("Cargo.toml")),
        )
        .arg("--")
        .arg("--emit=mir");

    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect(format!("Fail to compile {}.", benchmark.name).as_str())
        .wait()?;

    // find mir file
    let mut mir_file = None;
    for entry in read_dir(PathBuf::from(
        tmp_dir.path().join("target").join("release").join("deps"),
    ))? {
        let entry = entry?;

        if let Some(file_name) = entry.file_name().to_str() {
            if file_name.ends_with(".mir") {
                mir_file = Some(entry.path());
            }
        }
    }

    // if mir file found, extract mirs; else return err.
    if let Some(mir_file) = mir_file {
        read_mir(mir_file)
    } else {
        bail!(format!(
            "Mir file not found after {} compiled",
            benchmark.name
        ))
    }
}

fn do_analyze(mirs: Vec<MIR>, out_dir: &PathBuf) -> anyhow::Result<()> {
    count_mir_entry(&mirs, out_dir)?;

    Ok(())
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use crate::{
        benchmark::benchmark::{Benchamrk, BenchmarkConfig},
        toolchain::LocalToolchain,
    };

    use super::generate_mir;

    #[test]
    fn test_generate_mir() {
        let benchmark = Benchamrk {
            name: "condvar".to_string(),
            path: PathBuf::from("test/mir_analyze/run_analyze/benchmarks/condvar"),
            config: BenchmarkConfig {
                cargo_opts: None,
                cargo_rustc_opts: None,
                cargo_toml: None,
                compile_time_type: None,
                packages: None,
                runtime_test_type: None,
                example_lst: None,
                runtime_args: None,
                touch_file: None,
                disabled: false,
                runs: 0,
            },
        };

        generate_mir(
            &benchmark,
            &LocalToolchain {
                rustc: PathBuf::from("rustc"),
                cargo: PathBuf::from("cargo"),
                flame_graph: PathBuf::from(""),
                id: 0.to_string(),
            },
        )
        .unwrap();
    }
}
