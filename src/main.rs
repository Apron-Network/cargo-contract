// Copyright 2018-2021 Parity Technologies (UK) Ltd.
// This file is part of cargo-contract.
//
// cargo-contract is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// cargo-contract is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with cargo-contract.  If not, see <http://www.gnu.org/licenses/>.

#[cfg(test)]
#[cfg(feature = "integration-tests")]
mod tests;

use cargo_contract::{Opts, Command, BuildResult, Verbosity, OutputType};
use cargo_contract::{cmd};
use anyhow::{Error, Result};
use colored::Colorize;
use structopt::{clap, StructOpt};

fn main() {
    env_logger::init();

    let Opts::Contract(args) = Opts::from_args();
    match exec(args.cmd) {
        Ok(maybe_msg) => {
            if let Some(msg) = maybe_msg {
                println!("\t{}", msg)
            }
        }
        Err(err) => {
            eprintln!(
                "{} {}",
                "ERROR:".bright_red().bold(),
                format!("{:?}", err).bright_red()
            );
            std::process::exit(1);
        }
    }
}

fn exec(cmd: Command) -> Result<Option<String>> {
    match &cmd {
        Command::New { name, target_dir } => cmd::new::execute(name, target_dir.as_ref()),
        Command::Build(build) => {
            let result = build.exec()?;

            if matches!(result.output_type, OutputType::Json) {
                Ok(Some(result.serialize_json()?))
            } else if result.verbosity.is_verbose() {
                Ok(Some(result.display()))
            } else {
                Ok(None)
            }
        }
        Command::Check(check) => {
            let res = check.exec()?;
            assert!(
                res.dest_wasm.is_none(),
                "no dest_wasm must be on the generation result"
            );
            if res.verbosity.is_verbose() {
                Ok(Some(
                    "\nYour contract's code was built successfully.".to_string(),
                ))
            } else {
                Ok(None)
            }
        }
        Command::Test(test) => {
            let res = test.exec()?;
            if res.verbosity.is_verbose() {
                Ok(Some(res.display()?))
            } else {
                Ok(None)
            }
        }
        Command::Deploy(deploy) => {
            let (code_hash, contract) = deploy.exec()?;
            Ok(Some(format!(
                "Code hash: {:#x}, Contract account: {}",
                code_hash, contract
            )))
        }
        Command::Instantiate(instantiate) => {
            let contract_account = instantiate.run()?;
            Ok(Some(format!("Contract account: {}", contract_account)))
        }
        Command::Call(call) => {
            call.run()?;
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_result_seralization_sanity_check() {
        // given
        let raw_result = r#"{
  "dest_wasm": "/path/to/contract.wasm",
  "metadata_result": {
    "dest_metadata": "/path/to/metadata.json",
    "dest_bundle": "/path/to/contract.contract"
  },
  "target_directory": "/path/to/target",
  "optimization_result": {
    "dest_wasm": "/path/to/contract.wasm",
    "original_size": 64.0,
    "optimized_size": 32.0
  },
  "build_mode": "Debug",
  "build_artifact": "All",
  "verbosity": "Quiet"
}"#;

        let build_result = crate::BuildResult {
            dest_wasm: Some(PathBuf::from("/path/to/contract.wasm")),
            metadata_result: Some(crate::cmd::metadata::MetadataResult {
                dest_metadata: PathBuf::from("/path/to/metadata.json"),
                dest_bundle: PathBuf::from("/path/to/contract.contract"),
            }),
            target_directory: PathBuf::from("/path/to/target"),
            optimization_result: Some(crate::OptimizationResult {
                dest_wasm: PathBuf::from("/path/to/contract.wasm"),
                original_size: 64.0,
                optimized_size: 32.0,
            }),
            build_mode: Default::default(),
            build_artifact: Default::default(),
            verbosity: Verbosity::Quiet,
            output_type: OutputType::Json,
        };

        // when
        let serialized_result = build_result.serialize_json();

        // then
        assert!(serialized_result.is_ok());
        assert_eq!(serialized_result.unwrap(), raw_result);
    }
}
