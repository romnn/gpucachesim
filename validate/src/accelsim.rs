use super::materialize::{self, BenchmarkConfig};
use crate::{
    open_writable,
    options::{self, Options},
    RunError,
};
use accelsim::tracegen::{self, reader::Command as AccelsimCommand};
use color_eyre::{eyre, Help};
use itertools::Itertools;
use std::io::Write;
use std::path::Path;
use std::time::Duration;
use utils::fs::create_dirs;

fn convert_traces_to_json(
    trace_dir: &Path,
    kernelslist: &Path,
    mem_only: bool,
) -> eyre::Result<()> {
    let reader = utils::fs::open_readable(kernelslist)?;
    let accelsim_commands = tracegen::reader::read_commands(trace_dir, reader)?;

    let commands: Vec<_> = accelsim_commands
        .into_iter()
        .map(|cmd| match cmd {
            AccelsimCommand::MemcpyHtoD(memcopy) => {
                Ok::<_, eyre::Report>(trace_model::Command::MemcpyHtoD(memcopy))
            }
            AccelsimCommand::KernelLaunch((mut kernel, metadata)) => {
                // transform kernel instruction trace
                let kernel_trace_path = trace_dir.join(&kernel.trace_file);
                let reader = utils::fs::open_readable(kernel_trace_path)?;
                let parsed_trace = tracegen::reader::read_trace_instructions(
                    reader,
                    metadata.trace_version,
                    metadata.line_info,
                    mem_only,
                    &kernel,
                )?;

                let json_kernel_trace_name = format!("kernel-{}.json", kernel.id);
                let json_kernel_trace_path = trace_dir.join(&json_kernel_trace_name);
                let mut writer = utils::fs::open_writable(json_kernel_trace_path)?;

                serde_json::to_writer_pretty(&mut writer, &parsed_trace)?;

                // update the kernel trace path
                kernel.trace_file = json_kernel_trace_name;

                Ok::<_, eyre::Report>(trace_model::Command::KernelLaunch(kernel))
            }
        })
        .try_collect()?;

    let json_kernelslist = kernelslist.with_extension("json");
    serde_json::to_writer_pretty(utils::fs::open_writable(json_kernelslist)?, &commands)?;
    Ok(())
}

pub async fn trace(
    bench: &BenchmarkConfig,
    options: &Options,
    _trace_opts: &options::AccelsimTrace,
) -> Result<(), RunError> {
    let traces_dir = &bench.accelsim_trace.traces_dir;
    create_dirs(traces_dir).map_err(eyre::Report::from)?;

    let kernelslist = traces_dir.join("kernelslist.g");
    if !options.force && kernelslist.is_file() {
        return Err(RunError::Skipped);
    }

    let options = accelsim_trace::Options {
        traces_dir: traces_dir.clone(),
        nvbit_tracer_tool: None, // auto detect
        ..accelsim_trace::Options::default()
    };
    let dur = accelsim_trace::trace(&bench.executable, &bench.args, &options).await?;

    let trace_dur_file = traces_dir.join("trace_time.json");
    serde_json::to_writer_pretty(open_writable(trace_dur_file)?, &dur.as_millis())
        .map_err(eyre::Report::from)?;

    // convert accelsim traces to JSON for us to easily inspect
    let mem_only = false;
    convert_traces_to_json(traces_dir, &kernelslist, mem_only)?;
    // if let Err(err) = convert_traces_to_json(&traces_dir, &kernelslist) {
    //     log::error!(
    //         "failed to convert {} to JSON: {}",
    //         kernelslist.display(),
    //         err
    //     );
    // }
    Ok(())
}

pub async fn simulate_bench_config(
    bench: &BenchmarkConfig,
) -> Result<(async_process::Output, Duration), RunError> {
    let traces_dir = &bench.accelsim_trace.traces_dir;

    // todo: allow setting this in test-apps.yml ?
    let kernelslist = traces_dir.join("kernelslist.g");
    if !kernelslist.is_file() {
        return Err(RunError::Failed(
            eyre::eyre!("missing {}", kernelslist.display()).with_suggestion(|| {
                let trace_cmd = format!(
                    "cargo validate -b {}@{} accelsim-trace",
                    bench.name,
                    bench.input_idx + 1
                );
                format!("generate traces first using: `{trace_cmd}`")
            }),
        ));
    }

    let common = &bench.accelsim_simulate.common;

    let timeout = common.timeout.map(Into::into);

    let materialize::AccelsimSimConfigFiles {
        config,
        config_dir,
        trace_config,
        inter_config,
    } = bench.accelsim_simulate.configs.clone();

    let config = accelsim::SimConfig {
        config: Some(config),
        config_dir: Some(config_dir),
        trace_config: Some(trace_config),
        inter_config: Some(inter_config),
    };

    let extra_sim_args: &[String] = &[];
    let stream_output = false;
    let use_upstream = Some(true);
    let (log, dur) = accelsim_sim::simulate_trace(
        &traces_dir,
        &kernelslist,
        &config,
        timeout,
        extra_sim_args,
        stream_output,
        use_upstream,
    )
    .await?;
    Ok((log, dur))
}

pub async fn simulate(
    bench: &BenchmarkConfig,
    options: &Options,
    _sim_options: &options::AccelsimSim,
) -> Result<(), RunError> {
    let stats_dir = &bench.accelsim_simulate.stats_dir;

    if !options.force && crate::stats::already_exist(stats_dir) {
        return Err(RunError::Skipped);
    }

    let (output, dur) = simulate_bench_config(bench).await?;
    process_stats(output.stdout, dur, stats_dir)?;
    Ok(())
}

pub fn process_stats(
    log: impl AsRef<Vec<u8>>,
    dur: Duration,
    stats_dir: &Path,
) -> Result<(), RunError> {
    // parse stats
    let parse_options = accelsim::parser::Options::default();
    let log_reader = std::io::Cursor::new(log.as_ref());
    let stats = accelsim::parser::parse_stats(log_reader, &parse_options)?;

    create_dirs(stats_dir).map_err(eyre::Report::from)?;

    open_writable(stats_dir.join("log.txt"))?
        .write_all(log.as_ref())
        .map_err(eyre::Report::from)?;

    super::stats::write_csv_rows(
        open_writable(stats_dir.join("raw.stats.csv"))?,
        &stats.iter().collect::<Vec<_>>(),
    )?;

    let converted_stats: stats::Stats = stats.try_into()?;
    // dbg!(&converted_stats);
    crate::stats::write_stats_as_csv(stats_dir, converted_stats)?;

    #[cfg(debug_assertions)]
    let exec_time_file_path = stats_dir.join("exec_time.debug.json");
    #[cfg(not(debug_assertions))]
    let exec_time_file_path = stats_dir.join("exec_time.release.json");

    serde_json::to_writer_pretty(open_writable(exec_time_file_path)?, &dur.as_millis())
        .map_err(eyre::Report::from)?;
    Ok(())
}
