pub mod cmd;
pub mod crate_metadata;
pub mod util;
pub mod validate_wasm;
pub mod workspace;

use self::workspace::ManifestPath;

use crate::cmd::{metadata::MetadataResult, BuildCommand, CheckCommand, TestCommand};

use std::{
    convert::TryFrom,
    fmt::{Display, Formatter, Result as DisplayResult},
    path::PathBuf,
    str::FromStr,
};

use anyhow::{Error, Result};
use colored::Colorize;
use structopt::{clap, StructOpt};

use crate::cmd::{CallCommand, InstantiateCommand, InstantiateWithCode};
use sp_core::{crypto::Pair, sr25519};

#[derive(Debug, StructOpt)]
#[structopt(bin_name = "cargo")]
#[structopt(version = env!("CARGO_CONTRACT_CLI_IMPL_VERSION"))]
pub enum Opts {
    /// Utilities to develop Wasm smart contracts.
    #[structopt(name = "contract")]
    #[structopt(version = env!("CARGO_CONTRACT_CLI_IMPL_VERSION"))]
    #[structopt(setting = clap::AppSettings::UnifiedHelpMessage)]
    #[structopt(setting = clap::AppSettings::DeriveDisplayOrder)]
    #[structopt(setting = clap::AppSettings::DontCollapseArgsInUsage)]
    Contract(ContractArgs),
}

#[derive(Debug, StructOpt)]
pub struct ContractArgs {
    #[structopt(subcommand)]
    pub cmd: Command,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct HexData(pub Vec<u8>);

impl std::str::FromStr for HexData {
    type Err = hex::FromHexError;

    fn from_str(input: &str) -> std::result::Result<Self, Self::Err> {
        hex::decode(input).map(HexData)
    }
}

/// Arguments required for creating and sending an extrinsic to a substrate node
#[derive(Clone, Debug, StructOpt)]
pub struct ExtrinsicOpts {
    /// Websockets url of a substrate node
    #[structopt(
    name = "url",
    long,
    parse(try_from_str),
    default_value = "ws://localhost:9944"
    )]
    pub url: url::Url,
    /// Secret key URI for the account deploying the contract.
    #[structopt(name = "suri", long, short)]
    pub suri: String,
    /// Password for the secret key
    #[structopt(name = "password", long, short)]
    pub password: Option<String>,
    #[structopt(flatten)]
    pub verbosity: VerbosityFlags,
}

impl ExtrinsicOpts {
    pub fn signer(&self) -> Result<sr25519::Pair> {
        let password_override = self.password.as_ref().map(String::as_ref);
        println!("signer, suri: {}, password: {:?}", self.suri, password_override);
        sr25519::Pair::from_string(&self.suri, password_override)
            .map_err(|_| anyhow::anyhow!("Secret string error"))
    }

    /// Returns the verbosity
    pub fn verbosity(&self) -> Result<Verbosity> {
        TryFrom::try_from(&self.verbosity)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OptimizationPasses {
    Zero,
    One,
    Two,
    Three,
    Four,
    S,
    Z,
}

impl Display for OptimizationPasses {
    fn fmt(&self, f: &mut Formatter<'_>) -> DisplayResult {
        let out = match self {
            OptimizationPasses::Zero => "0",
            OptimizationPasses::One => "1",
            OptimizationPasses::Two => "2",
            OptimizationPasses::Three => "3",
            OptimizationPasses::Four => "4",
            OptimizationPasses::S => "s",
            OptimizationPasses::Z => "z",
        };
        write!(f, "{}", out)
    }
}

impl Default for OptimizationPasses {
    fn default() -> OptimizationPasses {
        OptimizationPasses::Z
    }
}

impl std::str::FromStr for OptimizationPasses {
    type Err = Error;

    fn from_str(input: &str) -> std::result::Result<Self, Self::Err> {
        // We need to replace " here, since the input string could come
        // from either the CLI or the `Cargo.toml` profile section.
        // If it is from the profile it could e.g. be "3" or 3.
        let normalized_input = input.replace("\"", "").to_lowercase();
        match normalized_input.as_str() {
            "0" => Ok(OptimizationPasses::Zero),
            "1" => Ok(OptimizationPasses::One),
            "2" => Ok(OptimizationPasses::Two),
            "3" => Ok(OptimizationPasses::Three),
            "4" => Ok(OptimizationPasses::Four),
            "s" => Ok(OptimizationPasses::S),
            "z" => Ok(OptimizationPasses::Z),
            _ => anyhow::bail!("Unknown optimization passes for option {}", input),
        }
    }
}

impl From<std::string::String> for OptimizationPasses {
    fn from(str: String) -> Self {
        OptimizationPasses::from_str(&str).expect("conversion failed")
    }
}

#[derive(Default, Clone, Debug, StructOpt)]
pub struct VerbosityFlags {
    /// No output printed to stdout
    #[structopt(long)]
    quiet: bool,
    /// Use verbose output
    #[structopt(long)]
    verbose: bool,
}

/// Denotes if output should be printed to stdout.
#[derive(Clone, Copy, serde::Serialize)]
pub enum Verbosity {
    /// Use default output
    Default,
    /// No output printed to stdout
    Quiet,
    /// Use verbose output
    Verbose,
}

impl Default for Verbosity {
    fn default() -> Self {
        Verbosity::Default
    }
}

impl Verbosity {
    /// Returns `true` if output should be printed (i.e. verbose output is set).
    pub fn is_verbose(&self) -> bool {
        match self {
            Verbosity::Quiet => false,
            Verbosity::Default | Verbosity::Verbose => true,
        }
    }
}

impl TryFrom<&VerbosityFlags> for Verbosity {
    type Error = Error;

    fn try_from(value: &VerbosityFlags) -> Result<Self, Self::Error> {
        match (value.quiet, value.verbose) {
            (false, false) => Ok(Verbosity::Default),
            (true, false) => Ok(Verbosity::Quiet),
            (false, true) => Ok(Verbosity::Verbose),
            (true, true) => anyhow::bail!("Cannot pass both --quiet and --verbose flags"),
        }
    }
}

#[derive(Default, Clone, Debug, StructOpt)]
pub struct UnstableOptions {
    /// Use the original manifest (Cargo.toml), do not modify for build optimizations
    #[structopt(long = "unstable-options", short = "Z", number_of_values = 1)]
    options: Vec<String>,
}

#[derive(Clone, Default)]
pub struct UnstableFlags {
    original_manifest: bool,
}

impl TryFrom<&UnstableOptions> for UnstableFlags {
    type Error = Error;

    fn try_from(value: &UnstableOptions) -> Result<Self, Self::Error> {
        let valid_flags = ["original-manifest"];
        let invalid_flags = value
            .options
            .iter()
            .filter(|o| !valid_flags.contains(&o.as_str()))
            .collect::<Vec<_>>();
        if !invalid_flags.is_empty() {
            anyhow::bail!("Unknown unstable-options {:?}", invalid_flags)
        }
        Ok(UnstableFlags {
            original_manifest: value.options.contains(&"original-manifest".to_owned()),
        })
    }
}

/// Describes which artifacts to generate
#[derive(Copy, Clone, Eq, PartialEq, Debug, StructOpt, serde::Serialize)]
#[structopt(name = "build-artifacts")]
pub enum BuildArtifacts {
    /// Generate the Wasm, the metadata and a bundled `<name>.contract` file
    #[structopt(name = "all")]
    All,
    /// Only the Wasm is created, generation of metadata and a bundled `<name>.contract` file is skipped
    #[structopt(name = "code-only")]
    CodeOnly,
    CheckOnly,
}

impl BuildArtifacts {
    /// Returns the number of steps required to complete a build artifact.
    /// Used as output on the cli.
    pub fn steps(&self) -> usize {
        match self {
            BuildArtifacts::All => 5,
            BuildArtifacts::CodeOnly => 3,
            BuildArtifacts::CheckOnly => 2,
        }
    }
}

impl std::str::FromStr for BuildArtifacts {
    type Err = String;
    fn from_str(artifact: &str) -> Result<Self, Self::Err> {
        match artifact {
            "all" => Ok(BuildArtifacts::All),
            "code-only" => Ok(BuildArtifacts::CodeOnly),
            _ => Err("Could not parse build artifact".to_string()),
        }
    }
}

impl Default for BuildArtifacts {
    fn default() -> Self {
        BuildArtifacts::All
    }
}

/// The mode to build the contract in.
#[derive(Eq, PartialEq, Copy, Clone, Debug, serde::Serialize)]
pub enum BuildMode {
    /// Functionality to output debug messages is build into the contract.
    Debug,
    /// The contract is build without any debugging functionality.
    Release,
}

impl Default for BuildMode {
    fn default() -> BuildMode {
        BuildMode::Debug
    }
}

impl Display for BuildMode {
    fn fmt(&self, f: &mut Formatter<'_>) -> DisplayResult {
        match self {
            Self::Debug => write!(f, "debug"),
            Self::Release => write!(f, "release"),
        }
    }
}

/// Use network connection to build contracts and generate metadata or use cached dependencies only.
#[derive(Eq, PartialEq, Copy, Clone, Debug, serde::Serialize)]
pub enum Network {
    /// Use network
    Online,
    /// Use cached dependencies.
    Offline,
}

impl Default for Network {
    fn default() -> Network {
        Network::Online
    }
}

impl Display for Network {
    fn fmt(&self, f: &mut Formatter<'_>) -> DisplayResult {
        match self {
            Self::Online => write!(f, ""),
            Self::Offline => write!(f, "--offline"),
        }
    }
}

/// The type of output to display at the end of a build.
pub enum OutputType {
    /// Output build results in a human readable format.
    HumanReadable,
    /// Output the build results JSON formatted.
    Json,
}

impl Default for OutputType {
    fn default() -> Self {
        OutputType::HumanReadable
    }
}

/// Result of the metadata generation process.
#[derive(serde::Serialize)]
pub struct BuildResult {
    /// Path to the resulting Wasm file.
    pub dest_wasm: Option<PathBuf>,
    /// Result of the metadata generation.
    pub metadata_result: Option<MetadataResult>,
    /// Path to the directory where output files are written to.
    pub target_directory: PathBuf,
    /// If existent the result of the optimization.
    pub optimization_result: Option<OptimizationResult>,
    /// The mode to build the contract in.
    pub build_mode: BuildMode,
    /// Which build artifacts were generated.
    pub build_artifact: BuildArtifacts,
    /// The verbosity flags.
    pub verbosity: Verbosity,
    /// The type of formatting to use for the build output.
    #[serde(skip_serializing)]
    pub output_type: OutputType,
}

/// Result of the optimization process.
#[derive(serde::Serialize)]
pub struct OptimizationResult {
    /// The path of the optimized wasm file.
    pub dest_wasm: PathBuf,
    /// The original Wasm size.
    pub original_size: f64,
    /// The Wasm size after optimizations have been applied.
    pub optimized_size: f64,
}

impl BuildResult {
    pub fn display(&self) -> String {
        let optimization = self.display_optimization();
        let size_diff = format!(
            "\nOriginal wasm size: {}, Optimized: {}\n\n",
            format!("{:.1}K", optimization.0).bold(),
            format!("{:.1}K", optimization.1).bold(),
        );
        debug_assert!(
            optimization.1 > 0.0,
            "optimized file size must be greater 0"
        );

        let build_mode = format!(
            "The contract was built in {} mode.\n\n",
            format!("{}", self.build_mode).to_uppercase().bold(),
        );

        if self.build_artifact == BuildArtifacts::CodeOnly {
            let out = format!(
                "{}{}Your contract's code is ready. You can find it here:\n{}",
                size_diff,
                build_mode,
                self.dest_wasm
                    .as_ref()
                    .expect("wasm path must exist")
                    .display()
                    .to_string()
                    .bold()
            );
            return out;
        };

        let mut out = format!(
            "{}{}Your contract artifacts are ready. You can find them in:\n{}\n\n",
            size_diff,
            build_mode,
            self.target_directory.display().to_string().bold(),
        );
        if let Some(metadata_result) = self.metadata_result.as_ref() {
            let bundle = format!(
                "  - {} (code + metadata)\n",
                util::base_name(&metadata_result.dest_bundle).bold()
            );
            out.push_str(&bundle);
        }
        if let Some(dest_wasm) = self.dest_wasm.as_ref() {
            let wasm = format!(
                "  - {} (the contract's code)\n",
                util::base_name(dest_wasm).bold()
            );
            out.push_str(&wasm);
        }
        if let Some(metadata_result) = self.metadata_result.as_ref() {
            let metadata = format!(
                "  - {} (the contract's metadata)",
                util::base_name(&metadata_result.dest_metadata).bold()
            );
            out.push_str(&metadata);
        }
        out
    }

    /// Returns a tuple of `(original_size, optimized_size)`.
    ///
    /// Panics if no optimization result is available.
    fn display_optimization(&self) -> (f64, f64) {
        let optimization = self
            .optimization_result
            .as_ref()
            .expect("optimization result must exist");
        (optimization.original_size, optimization.optimized_size)
    }

    /// Display the build results in a pretty formatted JSON string.
    pub fn serialize_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }
}

#[derive(Debug, StructOpt)]
pub enum Command {
    /// Setup and create a new smart contract project
    #[structopt(name = "new")]
    New {
        /// The name of the newly created smart contract
        name: String,
        /// The optional target directory for the contract project
        #[structopt(short, long, parse(from_os_str))]
        target_dir: Option<PathBuf>,
    },
    /// Compiles the contract, generates metadata, bundles both together in a `<name>.contract` file
    #[structopt(name = "build")]
    Build(BuildCommand),
    /// Check that the code builds as Wasm; does not output any `<name>.contract` artifact to the `target/` directory
    #[structopt(name = "check")]
    Check(CheckCommand),
    /// Test the smart contract off-chain
    #[structopt(name = "test")]
    Test(TestCommand),
    /// Upload the smart contract code to the chain
    #[structopt(name = "deploy")]
    Deploy(InstantiateWithCode),
    /// Instantiate a deployed smart contract
    Instantiate(InstantiateCommand),
    Call(CallCommand),
}