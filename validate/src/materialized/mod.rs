pub mod config;
pub mod value;

pub use config::{Config, GenericBenchmark as GenericBenchmarkConfig};

use super::{
    benchmark::paths::PathExt,
    template::{self, Render},
    Error, Target,
};
use indexmap::IndexMap;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub enum TargetBenchmarkConfig {
    Profile {
        profile_dir: PathBuf,
    },
    Trace {
        traces_dir: PathBuf,
        save_json: bool,
        full_trace: bool,
    },
    AccelsimTrace {
        /// Traces dir
        traces_dir: PathBuf,
    },
    Simulate {
        stats_dir: PathBuf,
        /// Traces dir (default to the dir specified in accelsim trace)
        traces_dir: PathBuf,
        accelsim_traces_dir: PathBuf,
        parallel: Option<bool>,
    },
    AccelsimSimulate {
        #[serde(flatten)]
        configs: config::AccelsimSimConfigFiles,
        /// Traces dir (default to the dir specified in accelsim trace)
        traces_dir: PathBuf,
        /// Stats dir
        stats_dir: PathBuf,
    },
    PlaygroundSimulate {
        #[serde(flatten)]
        configs: config::AccelsimSimConfigFiles,
        /// Traces dir (default to the dir specified in accelsim trace)
        traces_dir: PathBuf,
        /// Stats dir
        stats_dir: PathBuf,
    },
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct TemplateValues<B> {
    pub name: String,
    pub bench: B,
    pub input: super::matrix::Input,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct BenchmarkConfig {
    pub name: String,
    /// Relative index of the benchmark for this target.
    pub benchmark_idx: usize,
    pub uid: String,

    pub path: PathBuf,
    pub executable: PathBuf,

    /// Input values for this benchmark config.
    pub values: super::matrix::Input,
    /// Command line arguments for invoking the benchmark for this target.
    pub args: Vec<String>,
    /// Relative index of the input configuration for this target.
    pub input_idx: usize,

    pub common: config::GenericBenchmark,

    pub target: Target,
    pub target_config: TargetBenchmarkConfig,
}

impl std::fmt::Display for BenchmarkConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let executable = std::env::current_dir().ok().map_or_else(
            || self.executable.clone(),
            |cwd| self.executable.relative_to(cwd),
        );
        write!(
            f,
            "{} [{} {}]",
            self.name,
            executable.display(),
            self.args.join(" ")
        )
    }
}

#[must_use]
pub fn bench_config_name(name: &str, input: &super::matrix::Input, sort: bool) -> String {
    let mut bench_config_dir_name = Vec::new();
    let mut input: Vec<_> = input.clone().into_iter().collect();
    if sort {
        input.sort_by_key(|(k, _v)| k.clone());
    }
    for (k, v) in input {
        bench_config_dir_name.push(k);
        bench_config_dir_name.extend(value::flatten(v, sort).into_iter().map(|v| v.to_string()));
    }
    let bench_config_dir_name = bench_config_dir_name.join("-");
    format!("{}-{}", &name, bench_config_dir_name)
}

impl crate::Benchmark {
    #[allow(clippy::too_many_lines)]
    pub fn materialize_input(
        &self,
        name: String,
        input: super::matrix::Input,
        top_level_config: &config::Config,
        base: &Path,
        target: Target,
    ) -> Result<BenchmarkConfig, super::Error> {
        if !base.is_absolute() {
            return Err(super::Error::RelativeBase(base.to_path_buf()));
        }

        let values = TemplateValues {
            name: name.clone(),
            bench: self.clone(),
            input: input.clone(),
        };

        let cmd_args = self.args_template.render(&values)?;
        let cmd_args = super::benchmark::split_shell_command(cmd_args)?;

        let bench_uid = bench_config_name(&name, &input, true);
        let default_bench_dir = PathBuf::from(&name).join(&bench_uid);

        let top_level_common_config = match target {
            Target::Profile => &top_level_config.profile.common,
            Target::Trace => &top_level_config.trace.common,
            Target::AccelsimTrace => &top_level_config.accelsim_trace.common,
            Target::Simulate => &top_level_config.simulate.common,
            Target::AccelsimSimulate => &top_level_config.accelsim_simulate.common,
            Target::PlaygroundSimulate => &top_level_config.playground_simulate.common,
        };
        let common_config =
            self.config
                .clone()
                .materialize(base, Some(target), Some(&top_level_common_config))?;

        let trace_config = self.config.clone().materialize(
            base,
            Some(Target::Trace),
            Some(&top_level_config.trace.common),
        )?;
        let accelsim_trace_config = self.config.clone().materialize(
            base,
            Some(Target::AccelsimTrace),
            Some(&top_level_config.accelsim_trace.common),
        )?;

        let traces_dir = self
            .simulate
            .traces_dir
            .as_ref()
            .map(|template| template.render(&values))
            .transpose()?
            .map(|path| path.resolve(&trace_config.results_dir))
            .unwrap_or_else(|| {
                trace_config
                    .results_dir
                    .join(&default_bench_dir)
                    .join("trace")
            });

        let accelsim_traces_dir = self
            .simulate
            .accelsim_traces_dir
            .as_ref()
            .map(|template| template.render(&values))
            .transpose()?
            .map(|path| path.resolve(&trace_config.results_dir))
            .unwrap_or_else(|| {
                accelsim_trace_config
                    .results_dir
                    .join(&default_bench_dir)
                    .join("accelsim-trace")
            });

        let target_config = match target {
            Target::Profile => TargetBenchmarkConfig::Profile {
                profile_dir: common_config
                    .results_dir
                    .join(&default_bench_dir)
                    .join("profile"),
            },
            Target::Trace => TargetBenchmarkConfig::Trace {
                traces_dir,
                full_trace: top_level_config.trace.full_trace,
                save_json: top_level_config.trace.save_json,
            },
            Target::AccelsimTrace => TargetBenchmarkConfig::AccelsimTrace {
                traces_dir: accelsim_traces_dir,
            },
            Target::Simulate => TargetBenchmarkConfig::Simulate {
                stats_dir: common_config
                    .results_dir
                    .join(&default_bench_dir)
                    .join("sim"),
                traces_dir,
                accelsim_traces_dir,
                parallel: None,
            },
            Target::AccelsimSimulate => TargetBenchmarkConfig::AccelsimSimulate {
                stats_dir: common_config
                    .results_dir
                    .join(&default_bench_dir)
                    .join("accelsim-sim"),
                configs: self.accelsim_simulate.configs.materialize(
                    base,
                    top_level_config.accelsim_simulate.configs.clone(),
                    &values,
                )?,
                traces_dir: accelsim_traces_dir,
            },
            Target::PlaygroundSimulate => TargetBenchmarkConfig::PlaygroundSimulate {
                stats_dir: common_config
                    .results_dir
                    .join(&default_bench_dir)
                    .join("playground-sim"),
                configs: self.playground_simulate.configs.materialize(
                    base,
                    top_level_config.playground_simulate.configs.clone(),
                    &values,
                )?,
                traces_dir: accelsim_traces_dir,
            },
        };

        Ok(BenchmarkConfig {
            name,
            benchmark_idx: 0,
            input_idx: 0,
            uid: bench_uid,
            path: self.path.resolve(base),
            executable: self.executable().resolve(base),
            values: input,
            args: cmd_args,
            common: common_config,
            target,
            target_config,
        })
    }

    pub fn materialize_for_target(
        self,
        name: &str,
        base: &Path,
        config: &config::Config,
        target: Target,
    ) -> Result<Vec<BenchmarkConfig>, super::Error> {
        use serde_json_merge::{Dfs, Union};

        if !base.is_absolute() {
            return Err(super::Error::RelativeBase(base.to_path_buf()));
        }

        let mut target_matrix: serde_json::Value = serde_json::to_value(&self.matrix)?;
        let matrix_overrides = match target {
            Target::Simulate => serde_json::to_value(&self.simulate.inputs)?,
            Target::Profile => serde_json::to_value(&self.profile.inputs)?,
            Target::Trace => serde_json::to_value(&self.trace.inputs)?,
            Target::AccelsimTrace => serde_json::to_value(&self.accelsim_trace.inputs)?,
            Target::AccelsimSimulate => serde_json::to_value(&self.accelsim_simulate.inputs)?,
            Target::PlaygroundSimulate => serde_json::to_value(&self.playground_simulate.inputs)?,
        };

        target_matrix.union_recursive::<Dfs>(&matrix_overrides);

        let target_matrix: crate::matrix::Matrix = serde_json::from_value(target_matrix)?;

        let inputs: Result<Vec<_>, _> = target_matrix
            .expand()
            .into_iter()
            .enumerate()
            .map(|(input_idx, target_input)| {
                let mut bench_configs =
                    self.materialize_input(name.to_string(), target_input, config, base, target);
                for bench_config in bench_configs.iter_mut() {
                    bench_config.input_idx = input_idx;
                }
                bench_configs
            })
            .collect();
        inputs
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Benchmarks {
    pub config: config::Config,
    pub benchmarks: IndexMap<Target, IndexMap<String, Vec<BenchmarkConfig>>>,
}

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum QueryError {
    #[error("keys {unknown:?} not found in {valid:?}")]
    UnknownKeys {
        unknown: Vec<String>,
        valid: Vec<String>,
    },
}

impl Benchmarks {
    pub fn from_reader(reader: impl std::io::Read) -> Result<Self, super::DeserializeError> {
        let deser = serde_yaml::Deserializer::from_reader(reader);
        serde_path_to_error::deserialize(deser).map_err(|source| {
            let path = source.path().to_string();
            super::DeserializeError {
                source: source.into_inner(),
                path: Some(path),
            }
        })
    }

    pub fn get_input_configs(
        &self,
        target: Target,
        benchmark_name: String,
    ) -> impl Iterator<Item = &BenchmarkConfig> + '_ {
        self.benchmarks[&target]
            .iter()
            .filter(move |(name, _)| name.as_str() == benchmark_name.as_str())
            .flat_map(|(_, bench_configs)| bench_configs)
    }

    pub fn get_single_config(
        &self,
        target: Target,
        benchmark_name: impl Into<String>,
        input_idx: usize,
    ) -> Option<&BenchmarkConfig> {
        self.get_input_configs(target, benchmark_name.into())
            .find(|config| config.input_idx == input_idx)
    }

    pub fn query(
        &self,
        target: Target,
        benchmark_name: impl Into<String>,
        query: super::matrix::Input,
        strict: bool,
    ) -> impl Iterator<Item = Result<&BenchmarkConfig, QueryError>> + '_ {
        use serde_yaml::Value;
        use std::collections::HashSet;
        self.get_input_configs(target, benchmark_name.into())
            .filter_map(move |bench_config| {
                let bench_entries: HashSet<(&String, &Value)> =
                    bench_config.values.iter().collect();
                let bench_keys: HashSet<&String> = bench_config.values.keys().collect();

                let query_entries: HashSet<(&String, &Value)> = query.iter().collect();
                let query_keys: HashSet<&String> = query.keys().collect();

                let unknown_keys: Vec<_> = query_keys.difference(&bench_keys).collect();
                let intersecting_keys: Vec<_> = bench_keys.intersection(&query_keys).collect();
                let intersecting_entries: Vec<_> =
                    bench_entries.intersection(&query_entries).collect();

                let all_match = intersecting_entries.len() == intersecting_keys.len();
                if strict && !unknown_keys.is_empty() {
                    Some(Err(QueryError::UnknownKeys {
                        unknown: unknown_keys
                            .into_iter()
                            .copied()
                            .cloned()
                            .sorted()
                            .collect(),
                        valid: bench_keys.into_iter().cloned().sorted().collect(),
                    }))
                } else if all_match {
                    Some(Ok(bench_config))
                } else {
                    None
                }
            })
    }
}

impl crate::Benchmarks {
    // pub fn materialize_all_targets(
    //     self,
    //     name: &str,
    //     base: &Path,
    //     config: &config::Config,
    // ) -> Result<Vec<BenchmarkConfig>, super::Error> {
    //     use strum::IntoEnumIterator;
    //     let test = Target::iter()
    //         .flat_map(|target| self.materialize_for_target(base, target)?)
    //         .try_collect()?;
    //     Ok(test)
    //     // Ok(vec![])
    // }

    /// Materialize the full benchmark config file.
    ///
    /// This is the entry point for materialization.
    pub fn materialize(self, base: &Path) -> Result<Benchmarks, super::Error> {
        use strum::IntoEnumIterator;
        if !base.is_absolute() {
            return Err(super::Error::RelativeBase(base.to_path_buf()));
        }

        // this could be parallelized in case it is slow
        let config = self.config.materialize(base)?;
        let benchmarks: IndexMap<Target, IndexMap<String, Vec<BenchmarkConfig>>> = Target::iter()
            .map(|target| {
                // let config = config.clone();
                // dbg!(&target);
                let bench_configs: IndexMap<String, Vec<BenchmarkConfig>> = self
                    .benchmarks
                    .clone()
                    .into_iter()
                    .enumerate()
                    .map(|(benchmark_idx, (name, bench))| {
                        // dbg!(&target);
                        let mut bench_configs =
                            bench.materialize_for_target(&name, base, &config, target)?;
                        for bench_config in bench_configs.iter_mut() {
                            bench_config.benchmark_idx = benchmark_idx;
                        }
                        // dbg!(&name, bench_configs.len());
                        Ok::<_, super::Error>((name.clone(), bench_configs))
                    })
                    .try_collect()?;

                Ok::<_, super::Error>((target, bench_configs))
                // Ok((target, bench_configs))
            })
            .try_collect()?;
        // dbg!(benchmarks.keys().collect::<Vec<_>>());
        // dbg!(benchmarks.values().flatten().count());
        // dbg!(benchmarks
        //     .values()
        //     .flatten()
        //     .map(|bench| bench.target_config.clone())
        //     .collect::<Vec<_>>());
        Ok(Benchmarks { config, benchmarks })
    }
}

#[allow(clippy::unnecessary_wraps)]
#[cfg(test)]
mod tests {
    use super::Target;
    use color_eyre::eyre;
    use indexmap::IndexMap;
    use itertools::Itertools;
    use pretty_assertions_sorted as diff;
    use std::path::PathBuf;

    static INIT: std::sync::Once = std::sync::Once::new();

    pub fn init_test() {
        INIT.call_once(|| {
            env_logger::builder().is_test(true).init();
            color_eyre::install().unwrap();
        });
    }

    #[test]
    fn test_materialize_target_config() -> eyre::Result<()> {
        init_test();
        let base = PathBuf::from("/base");
        let parent_config = crate::materialized::GenericBenchmarkConfig {
            repetitions: 5,
            concurrency: Some(1),
            timeout: None,
            enabled: true,
            results_dir: PathBuf::from("results/"),
        };
        diff::assert_eq!(
            crate::GenericBenchmarkConfig {
                concurrency: Some(2),
                repetitions: None,
                timeout: None,
                enabled: None,
                results_dir: None,
            }
            .materialize(&base, None, Some(&parent_config))?,
            crate::materialized::config::GenericBenchmark {
                concurrency: Some(2),
                repetitions: 5,
                timeout: None,
                enabled: true,
                results_dir: PathBuf::from("/base/results"),
            }
        );
        Ok(())
    }

    #[test]
    fn test_materialize_config_invalid() -> eyre::Result<()> {
        init_test();
        let _base = PathBuf::from("/base");
        let config = r#"
results_dir: ../results
materialize_to: ./test-apps-materialized.yml
trace: {}
accelsim_trace: {}
profile: {}
simulate: {}
accelsim_simulate:
  config_dir: ./config_dir
  inter_config: ./inter.config

  # empty values are fine (will be resolved to the base dir)
  config: ""  

  # missing trace config
  # trace_config: ./trace.config
        "#;

        let result: Result<crate::Config, _> = serde_yaml::from_str(config);
        dbg!(&result);
        assert!(result.is_err());
        Ok(())
    }

    //     fn default_materialized_config(base: &Path) -> eyre::Result<crate::materialize::Config> {
    //         let config = r#"
    // results_dir: ../results
    // materialize_to: ./test-apps-materialized.yml
    // trace: {}
    // accelsim_trace: {}
    // profile: {}
    // simulate: {}
    // accelsim_simulate:
    //   config_dir: ./config_dir
    //   config: ./gpgpusim.config
    //   trace_config: ./trace.config
    //   inter_config: ./inter.config
    // "#;
    //
    //         let config: crate::Config = serde_yaml::from_str(config)?;
    //         let materialized = config.materialize(&base)?;
    //         Ok(materialized)
    //     }

    #[test]
    fn test_materialize_config_minimal() -> eyre::Result<()> {
        init_test();
        let base = PathBuf::from("/base");
        let config = r#"
results_dir: ../results
materialize_to: ./test-apps-materialized.yml
trace: {}
accelsim_trace: {}
profile: {}
simulate: {}
accelsim_simulate:
  config_dir: ./config_dir
  config: ./gpgpusim.config
  trace_config: ./trace.config
  inter_config: ./inter.config
        "#;

        let config: crate::Config = serde_yaml::from_str(config)?;
        let materialized = config.materialize(&base)?;
        dbg!(materialized);
        Ok(())
    }

    //     #[test]
    //     fn test_materialize_config() -> eyre::Result<()> {
    //         let base = PathBuf::from("/base");
    //         let config = r#"
    // results_dir: ../results
    // materialize_to: ./test-apps-materialized.yml
    // trace:
    //   # one benchmark at once to not stress the GPU
    //   concurrency: 1
    //   # tracing does not require multiple repetitions
    //   repetitions: 1
    // accelsim_trace:
    //   # one benchmark at once to not stress the GPU
    //   concurrency: 1
    //   # tracing does not require multiple repetitions
    //   repetitions: 1
    // profile:
    //   # one benchmark at once to not stress the GPU
    //   concurrency: 1
    //   # profile 5 repetitions to warm up the GPU
    //   repetitions: 5
    //   keep_log_file: true
    // # for simulation, we do not set a limit on concurrency
    // simulate:
    //   repetitions: 2
    // # for accelsim simulation, we do not set a limit on concurrency
    // accelsim_simulate:
    //   repetitions: 2
    //         "#;
    //
    //         let config: crate::Config = serde_yaml::from_str(config)?;
    //         let materialized = config.materialize(&base)?;
    //         dbg!(materialized);
    //         assert!(false);
    //
    //         // let parent_config = TargetConfig {
    //         //     repetitions: Some(5),
    //         //     concurrency: None,
    //         //     results_dir: Some(PathBuf::from("results/")),
    //         // };
    //         // diff::assert_eq!(
    //         //     crate::TargetConfig {
    //         //         concurrency: Some(2),
    //         //         repetitions: None,
    //         //         results_dir: None,
    //         //     }
    //         //     .materialize(&base, &parent_config)?,
    //         //     TargetConfig {
    //         //         concurrency: Some(2),
    //         //         repetitions: Some(5),
    //         //         results_dir: Some(PathBuf::from("/base/results")),
    //         //     }
    //         // );
    //         Ok(())
    //     }

    #[test]
    fn test_materialize_benchmark() -> eyre::Result<()> {
        init_test();
        let base = PathBuf::from("/base");
        let config = r#"
results_dir: ./results
materialize_to: ./test-apps-materialized.yml
trace:
  concurrency: 1
  repetitions: 1
accelsim_trace:
  results_dir: ./accel-trace-results
  concurrency: 1
  repetitions: 1
profile:
  results_dir: ./profile-results
  concurrency: 1
  repetitions: 5
  keep_log_file: true
simulate:
  results_dir: ./results
  repetitions: 2
accelsim_simulate:
  results_dir: ./results
  repetitions: 2
  config_dir: ./config_dir
  config: ./gpgpusim.config
  trace_config: ./trace.config
  inter_config: ./inter.config
        "#;
        let config: crate::Config = serde_yaml::from_str(config)?;
        let materialized_config = config.materialize(&base)?;
        dbg!(&materialized_config);

        let benchmark = r#"
path: ./vectoradd
executable: vectoradd
inputs:
  data_type: [32]
  length: [100, 1000, 10000]
  single_value: "this is added to all inputs"
args: "{{input.length}} {{input.data_type}}"
accelsim_simulate:
  trace_config: "./my/configs/{{ name }}-{{ input.data_type }}.config"
  inter_config: "/absolute//configs/{{ name }}-{{ input.data_type }}.config"
  custom_template: "{{ input.single_value }}"
profile:
  # currently, log_file and metrics_file are not used :(
  log_file: "./my-own-path/{{ name }}/{{ bench.custom }}-{{ length }}-{{ data_type }}/nvprof.log"
  metrics_file: "./results/vectorAdd/vectorAdd-32-100/metrics.json"
custom: "hello {{ bench.other }}"
other: "hello"
"#;

        let benchmark: crate::Benchmark = serde_yaml::from_str(benchmark)?;
        // let materialized =
        //     benchmark.materialize_for_target("vectorAdd", &base, &materialized_config)?;
        let materialized = benchmark.materialize_for_target(
            "vectorAdd",
            &base,
            &materialized_config,
            crate::Target::AccelsimSimulate,
        )?;

        dbg!(&materialized);

        diff::assert_eq!(
            materialized[0].values,
            serde_yaml::from_str::<IndexMap<String, serde_yaml::Value>>(
                r#"
"data_type": 32
"length": 100
"single_value": "this is added to all inputs""#
            )?,
            "expanded both singular and multiple input values in the correct order",
        );
        diff::assert_eq!(
            materialized[0].args,
            vec!["100", "32"],
            "templated and split shell args correctly"
        );
        diff::assert_eq!(
            materialized[0].executable,
            PathBuf::from("/base/vectoradd/vectoradd"),
            "resolved path to executable"
        );
        // todo!();
        // diff::assert_eq!(
        //     materialized[0].accelsim_simulate.configs.trace_config,
        //     PathBuf::from("/base/my/configs/vectorAdd-32.config"),
        //     "used custom template for the trace config"
        // );
        // diff::assert_eq!(
        //     materialized[0].accelsim_simulate.configs.trace_config,
        //     PathBuf::from("/base/my/configs/vectorAdd-32.config"),
        //     "used custom template for the trace config"
        // );
        // diff::assert_eq!(
        //     materialized[0].accelsim_simulate.configs.inter_config,
        //     PathBuf::from("/absolute/configs/vectorAdd-32.config"),
        //     "used custom template with absolute path for the inter config"
        // );

        // TODO: make use of the additional values and see if / how they can be used
        Ok(())
    }

    #[test]
    fn test_query_benchmarks() -> eyre::Result<()> {
        init_test();
        let base = PathBuf::from("/base");
        let benchmarks = r#"
config:
  results_dir: ../results

benchmarks:
  vectorAdd:
    path: ./vectoradd
    executable: vectoradd
    inputs:
      dtype: [32]
      length: [100, 1000, 10000]
    args: "{{ input.length }} {{ input.dtype }}"
"#;

        let benchmark: crate::Benchmarks = serde_yaml::from_str(benchmarks)?;
        // dbg!(&benchmark);
        let materialized = benchmark.materialize(&base)?;

        macro_rules! query {
            ($query:expr) => {{
                $query
                    .map_ok(|bench_config| bench_config.uid.clone())
                    .sorted()
                    .collect::<Vec<Result<String, super::QueryError>>>()
            }};
        }

        diff::assert_eq!(
            query!(materialized.query(
                Target::Simulate,
                "invalid bench name",
                crate::input!({}),
                false
            )),
            vec![] as Vec::<Result<String, super::QueryError>>
        );

        let all_vectoradd_configs: Vec<Result<String, super::QueryError>> = vec![
            Ok("vectorAdd-dtype-32-length-100".to_string()),
            Ok("vectorAdd-dtype-32-length-1000".to_string()),
            Ok("vectorAdd-dtype-32-length-10000".to_string()),
        ];

        diff::assert_eq!(
            query!(materialized.query(Target::Simulate, "vectorAdd", crate::input!({}), false)),
            all_vectoradd_configs,
        );
        diff::assert_eq!(
            query!(materialized.query(
                Target::Simulate,
                "vectorAdd",
                crate::input!({ "dtype": 32 }),
                false
            )),
            all_vectoradd_configs
        );
        diff::assert_eq!(
            query!(materialized.query(
                Target::Simulate,
                "vectorAdd",
                crate::input!({ "invalid key": 32 }),
                false
            )),
            all_vectoradd_configs
        );
        diff::assert_eq!(
            query!(materialized.query(
                Target::Simulate,
                "vectorAdd",
                crate::input!({ "invalid key": 32 }),
                true
            )),
            vec![
                Err(super::QueryError::UnknownKeys {
                    unknown: vec!["invalid key".to_string()],
                    valid: vec!["dtype".to_string(), "length".to_string()],
                });
                3
            ] as Vec::<Result<String, super::QueryError>>
        );

        diff::assert_eq!(
            query!(materialized.query(
                Target::Simulate,
                "vectorAdd",
                crate::input!({ "length": 100 }),
                false
            )),
            vec![Ok("vectorAdd-dtype-32-length-100".to_string())]
                as Vec::<Result<String, super::QueryError>>
        );
        diff::assert_eq!(
            query!(materialized.query(
                Target::Simulate,
                "vectorAdd",
                crate::input!({ "dtype": 32, "length": 1000 }),
                true
            )),
            vec![Ok("vectorAdd-dtype-32-length-1000".to_string())]
                as Vec::<Result<String, super::QueryError>>
        );
        diff::assert_eq!(
            query!(materialized.query(
                Target::Simulate,
                "vectorAdd",
                crate::input!({ "unknown": 32, "length": 1000 }),
                false
            )),
            vec![Ok("vectorAdd-dtype-32-length-1000".to_string())]
                as Vec::<Result<String, super::QueryError>>
        );
        Ok(())
    }
}
