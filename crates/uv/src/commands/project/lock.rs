use anstream::eprint;
use anyhow::Result;

use distribution_types::IndexLocations;
use install_wheel_rs::linker::LinkMode;
use uv_cache::Cache;
use uv_client::{BaseClientBuilder, RegistryClientBuilder};
use uv_configuration::{
    Concurrency, ConfigSettings, ExtrasSpecification, NoBinary, NoBuild, PreviewMode, Reinstall,
    SetupPyStrategy, Upgrade,
};
use uv_dispatch::BuildDispatch;
use uv_interpreter::PythonEnvironment;
use uv_requirements::{ProjectWorkspace, RequirementsSpecification};
use uv_resolver::{FlatIndex, InMemoryIndex, Lock, Options};
use uv_types::{BuildIsolation, EmptyInstalledPackages, HashStrategy, InFlight};
use uv_warnings::warn_user;

use crate::commands::project::ProjectError;
use crate::commands::{pip, project, ExitStatus};
use crate::printer::Printer;

/// Resolve the project requirements into a lockfile.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn lock(
    preview: PreviewMode,
    cache: &Cache,
    printer: Printer,
) -> Result<ExitStatus> {
    if preview.is_disabled() {
        warn_user!("`uv lock` is experimental and may change without warning.");
    }

    // Find the project requirements.
    let project = ProjectWorkspace::discover(std::env::current_dir()?).await?;

    // Discover or create the virtual environment.
    let venv = project::init_environment(&project, preview, cache, printer)?;

    // Perform the lock operation.
    match do_lock(&project, &venv, preview, cache, printer).await {
        Ok(_) => Ok(ExitStatus::Success),
        Err(ProjectError::Operation(pip::operations::Error::Resolve(
            uv_resolver::ResolveError::NoSolution(err),
        ))) => {
            let report = miette::Report::msg(format!("{err}"))
                .context("No solution found when resolving dependencies:");
            eprint!("{report:?}");
            Ok(ExitStatus::Failure)
        }
        Err(err) => Err(err.into()),
    }
}

/// Lock the project requirements into a lockfile.
pub(super) async fn do_lock(
    project: &ProjectWorkspace,
    venv: &PythonEnvironment,
    preview: PreviewMode,
    cache: &Cache,
    printer: Printer,
) -> Result<Lock, ProjectError> {
    // TODO(zanieb): Support client configuration
    let client_builder = BaseClientBuilder::default();

    // Read all requirements from the provided sources.
    // TODO(zanieb): Consider allowing constraints and extras
    // TODO(zanieb): Allow specifying extras somehow
    let spec = RequirementsSpecification::from_sources(
        // TODO(konsti): With workspace (just like with extras), these are the requirements for
        // syncing. For locking, we want to use the entire workspace with all extras.
        // See https://github.com/astral-sh/uv/issues/3700
        &project.requirements(),
        &[],
        &[],
        None,
        &ExtrasSpecification::None,
        &client_builder,
        preview,
    )
    .await?;

    // Determine the tags, markers, and interpreter to use for resolution.
    let interpreter = venv.interpreter().clone();
    let tags = venv.interpreter().tags()?;
    let markers = venv.interpreter().markers();

    // Initialize the registry client.
    // TODO(zanieb): Support client options e.g. offline, tls, etc.
    let client = RegistryClientBuilder::new(cache.clone())
        .markers(markers)
        .platform(venv.interpreter().platform())
        .build();

    // TODO(charlie): Respect project configuration.
    let build_isolation = BuildIsolation::default();
    let concurrency = Concurrency::default();
    let config_settings = ConfigSettings::default();
    let extras = ExtrasSpecification::default();
    let flat_index = FlatIndex::default();
    let hasher = HashStrategy::default();
    let in_flight = InFlight::default();
    let index = InMemoryIndex::default();
    let index_locations = IndexLocations::default();
    let link_mode = LinkMode::default();
    let no_binary = NoBinary::default();
    let no_build = NoBuild::default();
    let options = Options::default();
    let reinstall = Reinstall::default();
    let setup_py = SetupPyStrategy::default();
    let upgrade = Upgrade::default();

    // Create a build dispatch.
    let build_dispatch = BuildDispatch::new(
        &client,
        cache,
        &interpreter,
        &index_locations,
        &flat_index,
        &index,
        &in_flight,
        setup_py,
        &config_settings,
        build_isolation,
        link_mode,
        &no_build,
        &no_binary,
        concurrency,
    );

    // Resolve the requirements.
    let resolution = pip::operations::resolve(
        spec.requirements,
        spec.constraints,
        spec.overrides,
        spec.source_trees,
        spec.project,
        &extras,
        EmptyInstalledPackages,
        &hasher,
        &reinstall,
        &upgrade,
        &interpreter,
        tags,
        markers,
        &client,
        &flat_index,
        &index,
        &build_dispatch,
        concurrency,
        options,
        printer,
    )
    .await?;

    // Notify the user of any resolution diagnostics.
    pip::operations::diagnose_resolution(resolution.diagnostics(), printer)?;

    // Write the lockfile to disk.
    let lock = resolution.lock()?;
    let encoded = toml::to_string_pretty(&lock)?;
    fs_err::tokio::write(
        project.workspace().root().join("uv.lock"),
        encoded.as_bytes(),
    )
    .await?;

    Ok(lock)
}
