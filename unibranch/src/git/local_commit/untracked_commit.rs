use std::fmt::Debug;

use anyhow::Context;
use git2::{Commit, Repository};

use crate::git::{local_commit::CommitMetadata, GitRepo};

use super::TrackedCommit;

pub struct UnTrackedCommit<'repo> {
    repo: &'repo Repository,
    git_repo: &'repo GitRepo,
    commit: Commit<'repo>,
}

impl<'repo> UnTrackedCommit<'repo> {
    pub fn new(repo: &'repo Repository, git_repo: &'repo GitRepo, commit: Commit<'repo>) -> Self {
        Self {
            repo,
            git_repo,
            commit,
        }
    }
    pub fn as_commit(&self) -> &Commit<'_> {
        &self.commit
    }

    pub fn commit(self) -> Commit<'repo> {
        self.commit
    }

    pub(crate) fn rebase(self, parent_commit: &Commit<'_>) -> anyhow::Result<Self> {
        let mut index = self
            .repo
            .cherrypick_commit(self.as_commit(), parent_commit, 0, None)?;
        let new_commit = {
            let signature = self.as_commit().author();
            let tree_id = index.write_tree_to(self.repo)?;
            let tree = self.repo.find_tree(tree_id)?;
            let new_commit_id = self.repo.commit(
                None,
                &signature,
                &signature,
                self.commit.message().expect("Not valid UTF-8 message"),
                &tree,
                &[parent_commit],
            )?;
            self.repo.find_commit(new_commit_id)?
        };
        Ok(UnTrackedCommit {
            repo: self.repo,
            git_repo: self.git_repo,
            commit: new_commit,
        })
    }

    pub(crate) fn track(self, branch_name: Option<String>) -> anyhow::Result<TrackedCommit<'repo>> {
        let commit_msg = self
            .as_commit()
            .message()
            .context("Commit message is not valid UTF-8")?;

        let branch_name =
            branch_name.unwrap_or_else(|| self.generate_remote_branch_name(commit_msg));
        let origin_main_commit = self.git_repo.base_commit()?;
        let mut complete_index = self
            .repo
            .cherrypick_commit(self.as_commit(), &origin_main_commit, 0, None)
            .context("Cherry picking directly on master")?;

        if complete_index.has_conflicts() {
            anyhow::bail!("There are conflicts");
        }

        let tree_id = complete_index.write_tree_to(self.repo)?;
        let tree = self.repo.find_tree(tree_id)?;

        let remote_commit = {
            let signature = self.as_commit().author();
            self.repo.commit(
                None,
                &signature,
                &signature,
                commit_msg,
                &tree,
                &[&origin_main_commit],
            )?
        };

        //Create meta_data
        let meta_data = CommitMetadata {
            remote_branch_name: std::borrow::Cow::Owned(branch_name),
            remote_commit,
        };
        self.git_repo.save_meta_data(self.as_commit(), &meta_data)?;
        Ok(TrackedCommit::new(
            self.repo,
            self.git_repo,
            self.commit,
            meta_data,
        ))
    }

    fn generate_remote_branch_name(&self, commit_msg: &str) -> String {
        let branch_name = {
            let title = commit_msg
                .lines()
                .next()
                .expect("Must have at least one line");
            title
                .replace(
                    |c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'),
                    "-",
                )
                .to_ascii_lowercase()
        };
        branch_name
    }
}

impl Debug for UnTrackedCommit<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let commit = &self.commit;
        write!(
            f,
            "Untracked Commit: {:?} {:?}",
            commit.id(),
            commit.message()
        )
    }
}
