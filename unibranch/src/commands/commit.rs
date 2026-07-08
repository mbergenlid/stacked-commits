use crate::git::{local_commit::MainCommit, GitRepo};

#[derive(Default, clap::Parser)]
pub struct Options {
    pub commit_ref: Option<String>,
}

pub fn execute(config: Options, git_repo: GitRepo) -> anyhow::Result<()> {
    // Take the index and commit it to the remote branch
    //
    let rev = config.commit_ref.unwrap_or_else(|| "HEAD".to_string());

    let main_commit = git_repo.find_unpushed_commit(&rev)?;
    let main_commit_id = main_commit.id();
    let new_rebase = match main_commit {
        MainCommit::UnTracked(_) => todo!(),
        MainCommit::Tracked(tracked_commit) => tracked_commit.commit_staged()?,
    };
    let mut parent_commit_id = new_rebase.commit();
    for original_commit in git_repo
        .unpushed_commits()?
        .into_iter()
        .skip_while(|c| c.id() != main_commit_id)
        .skip(1)
    {
        println!("Rebasing commit: {}", original_commit.message().unwrap());
        match original_commit {
            MainCommit::UnTracked(local_commit) => {
                let rebased_commit = local_commit.rebase(&parent_commit_id)?;
                parent_commit_id = rebased_commit.commit();
            }
            MainCommit::Tracked(tracked_commit) => {
                let rebased_commit = tracked_commit.rebase(&parent_commit_id)?;
                parent_commit_id = rebased_commit.commit();
            }
        };
    }
    git_repo.update_current_branch(&parent_commit_id)?;
    Ok(())
}
