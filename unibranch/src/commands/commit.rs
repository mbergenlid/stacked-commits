use crate::git::{local_commit::MainCommit, GitRepo};

#[derive(Default, clap::Parser)]
pub struct Options;

pub fn execute(_config: Options, git_repo: GitRepo) -> anyhow::Result<()> {
    // Take the index and commit it to the remote branch
    //

    let main_commit = git_repo.find_unpushed_commit("HEAD")?;
    match main_commit {
        MainCommit::UnTracked(_) => todo!(),
        MainCommit::Tracked(tracked_commit) => {
            tracked_commit.commit_staged()?;
        }
    }
    Ok(())
}
