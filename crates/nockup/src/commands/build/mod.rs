#[path = "build.rs"]
mod builder_impl;
pub mod init;
pub mod run;

use anyhow::Result;

use crate::cli::ProjectCommand;

pub async fn run(cmd: ProjectCommand) -> Result<()> {
    match cmd {
        ProjectCommand::Build { project } => {
            let project = project.as_deref().unwrap_or(".");
            builder_impl::run(project).await
        }
        ProjectCommand::Run { project, bin, args } => {
            let project = project.as_deref().unwrap_or(".");
            run::run(project.to_string(), bin, args).await
        }
        ProjectCommand::Init => init::run().await,
    }
}
