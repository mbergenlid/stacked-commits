use test_repo::{RemoteRepo, TestRepoWithRemote};

use crate::{commands::create, git::GitRepo};

use super::{MainCommit, TrackedCommit};

mod tracked_commit_merge_remote_head;
mod tracked_commit_sync_with_main;
mod tracked_commit_update_local_branch_head;
mod untracked_commit_rebase;

fn tracked(commit: MainCommit<'_>) -> TrackedCommit<'_> {
    match commit {
        MainCommit::UnTracked(_) => panic!("not a TrackedCommit"),
        MainCommit::Tracked(tracked) => tracked,
    }
}

fn setup_repo(remote: &RemoteRepo) -> TestRepoWithRemote<'_> {
    let local = remote.clone_repo();

    let local = local
        .create_file("file1", "Hello, World!")
        .commit_all("Initial")
        .push();

    let local = local
        .create_file("file2", "another file")
        .commit_all("Commit 1");

    {
        create::execute(
            create::Options::default(),
            GitRepo::open(local.path()).unwrap(),
        )
        .unwrap();
    }
    local
}
