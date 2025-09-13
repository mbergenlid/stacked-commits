use std::{ffi::CString, fmt::Debug};

use anyhow::Context;
use anyhow::Ok;
use git2::ApplyOptions;
use git2::Diff;
use git2::DiffDelta;
use git2::Index;
use git2::MergeOptions;
use git2::{Branch, Commit, Oid, Repository};
use indoc::formatdoc;
use tracing::info;

use crate::git::SyncState;

use super::CommitMetadata;
use super::GitRepo;
use super::UnTrackedCommit;

#[derive(Clone)]
pub struct TrackedCommit<'repo> {
    repo: &'repo Repository,
    git_repo: &'repo GitRepo,
    commit: Commit<'repo>,
    meta_data: CommitMetadata<'repo>,
}

impl<'repo> TrackedCommit<'repo> {
    pub fn new(
        repo: &'repo Repository,
        git_repo: &'repo GitRepo,
        commit: Commit<'repo>,
        meta_data: CommitMetadata<'repo>,
    ) -> Self {
        Self {
            repo,
            git_repo,
            commit,
            meta_data,
        }
    }

    pub fn remote_branch(&self) -> anyhow::Result<Branch> {
        let remote_branch = self
            .repo
            .find_branch(
                &format!("origin/{}", self.meta_data.remote_branch_name),
                git2::BranchType::Remote,
            )
            .context("Find the remote branch")?;
        Ok(remote_branch)
    }

    pub fn local_branch_head(&self) -> anyhow::Result<Commit> {
        let commit_meta_data = &self.meta_data;
        Ok(self.repo.find_commit(commit_meta_data.remote_commit)?)
    }

    pub fn as_commit(&self) -> &Commit {
        &self.commit
    }

    pub fn commit(self) -> Commit<'repo> {
        self.commit
    }

    pub fn meta_data(&self) -> &CommitMetadata {
        &self.meta_data
    }

    //
    // Apply the diff between this commit and the self.meta_data.remote_commit
    // and return the new TrackedCommit
    //
    //
    //              *
    //              |    * (Merge)
    //              |   / \
    //              *  /   * (remote_branch_head)
    //              | * <-/------------------------ cherry-pick c1 local_branch_head (resolve conflicts by accepting theirs)
    //              |  \ /
    //        c1    *   * (local_branch_head)
    //              |  /
    //              | /
    //  (origin)    *
    pub fn update_local_branch_head(self) -> anyhow::Result<Self> {
        let remote_commit = self.repo.find_commit(self.meta_data().remote_commit)?;

        info!("Sync with branch head: {}", remote_commit.id());

        let origin_main_commit = self.git_repo.base_commit()?;
        let complete_index = self
            .repo
            .cherrypick_commit(
                self.as_commit(),
                &origin_main_commit,
                0,
                Some(MergeOptions::default().file_favor(git2::FileFavor::Theirs)),
            )
            .context("Cherry picking directly on master")?;

        if complete_index.has_conflicts() {
            anyhow::bail!("There are conflicts");
        }

        let patch = self.repo.diff_tree_to_index(
            Some(&remote_commit.tree()?),
            Some(&complete_index),
            None,
        )?;
        // Split the patch
        let main_sync_patch = self.repo.diff_tree_to_tree(
            Some(&remote_commit.tree()?),
            Some(&origin_main_commit.tree()?),
            None,
        )?;

        let mut files_in_main_patch = Vec::new();
        main_sync_patch.foreach(
            &mut |file_delta, _| {
                files_in_main_patch.push((file_delta.old_file().id(), file_delta.new_file().id()));
                true
            },
            None,
            Some(&mut |_, _| true),
            None,
        )?;

        println!("Main patch files: {files_in_main_patch:?}");

        let new_commit = self.split_and_apply_patch(remote_commit, &patch, |delta| {
            if let Some(delta) = delta {
                files_in_main_patch.contains(&(delta.old_file().id(), delta.new_file().id()))
            } else {
                panic!("delta callback without any DiffDelta");
            }
        })?;

        if new_commit.is_none() {
            drop(new_commit);
            return std::result::Result::Ok(self);
        }

        let new_commit = new_commit.unwrap();
        let new_commit_id = new_commit.id();
        drop(new_commit);
        info!("New patch commit {}", new_commit_id);
        let new_meta = self.meta_data.update_commit(new_commit_id);
        self.git_repo.save_meta_data(&self.commit, &new_meta)?;
        std::result::Result::Ok(TrackedCommit {
            repo: self.repo,
            git_repo: self.git_repo,
            commit: self.commit,
            meta_data: new_meta,
        })
    }

    fn split_and_apply_patch<F>(
        &self,
        parent: Commit,
        patch: &Diff,
        mut delta_cb: F,
    ) -> anyhow::Result<Option<Commit<'_>>>
    where
        F: FnMut(Option<DiffDelta<'_>>) -> bool,
    {
        let mut new_index = self
            .repo
            .apply_to_tree(
                &parent.tree()?,
                patch,
                Some(ApplyOptions::new().delta_callback(|delta| delta_cb(delta))),
            )
            .context("Apply commit patch to old branch")?;

        let main_sync_commit = self
            .commit_index(&mut new_index, &parent, "Sync with main!")?
            .unwrap_or(parent);

        let mut index2 = self
            .repo
            .apply_to_tree(
                &main_sync_commit.tree()?,
                patch,
                Some(ApplyOptions::new().delta_callback(|delta| !delta_cb(delta))),
            )
            .context("Apply commit patch to old branch")?;

        self.commit_index(&mut index2, &main_sync_commit, "Fixup!")
    }

    fn commit_index(
        &self,
        index: &mut Index,
        parent: &Commit,
        msg: &str,
    ) -> anyhow::Result<Option<Commit<'_>>> {
        if index.has_conflicts() {
            for c in index.conflicts()? {
                let c = c?;
                println!(
                    "{} {} {}",
                    c.our
                        .as_ref()
                        .map(|our| String::from_utf8(our.path.clone()).unwrap())
                        .unwrap_or("NONE".to_string()),
                    c.their
                        .map(|our| String::from_utf8(our.path).unwrap())
                        .unwrap_or("NONE".to_string()),
                    c.ancestor
                        .map(|our| String::from_utf8(our.path).unwrap())
                        .unwrap_or("NONE".to_string())
                );
            }
            panic!("Conflicts while cherry-picking");
        }
        if index.is_empty() {
            return std::result::Result::Ok(None);
        }
        let tree_id = index.write_tree_to(self.repo)?;
        if tree_id == parent.tree()?.id() {
            return std::result::Result::Ok(None);
        }
        let tree = self.repo.find_tree(tree_id)?;
        let new_commit = {
            let signature = self.as_commit().author();
            self.repo
                .commit(None, &signature, &signature, msg, &tree, &[parent])?
        };

        std::result::Result::Ok(Some(self.repo.find_commit(new_commit)?))
    }

    ///
    /// Merge remote_branch_head with local_branch_head unless remote_branch_head any
    /// of those are a direct dependant on the other.
    ///
    /// Will not update from remote.
    /// ```text
    ///                 *
    ///                 |    * (Merge) <---- Produces this merge unless.
    ///                 |   / \
    ///                 *  /   * (remote_branch_head)
    ///                 | * <-/------------------------ (local_branch_head)
    ///                 |  \ /
    ///           c1    *   *
    ///                 |  /
    ///                 | /
    ///     (origin)    *
    /// ```
    pub fn merge_remote_head(self, new_parent: Option<&Commit>) -> anyhow::Result<Self> {
        // TODO: This should not take in a parent. The rebase should happen after
        let remote_branch_commit = self.remote_branch()?.get().peel_to_commit()?;
        let remote_branch_head = remote_branch_commit.id();
        let local_branch_head = self.meta_data().remote_commit;
        let merge_base = self
            .repo
            .merge_base(local_branch_head, remote_branch_head)?;

        let new_remote_commit = if merge_base == local_branch_head {
            self.repo.find_commit(remote_branch_head)?
        } else if merge_base == remote_branch_head {
            drop(remote_branch_commit);
            return Ok(self);
        } else {
            let local_branch_commit = self.repo.find_commit(local_branch_head)?;
            let oid = self.merge(&local_branch_commit, &remote_branch_commit)?;
            self.repo.find_commit(oid)?
        };

        let new_remote_tree = new_remote_commit.tree()?;
        let diff = self.repo.diff_tree_to_tree(
            Some(&self.git_repo.base_commit()?.tree()?),
            Some(&new_remote_tree),
            None,
        )?;

        let parent_commit = if let Some(parent) = new_parent {
            parent.clone()
        } else {
            self.commit.parent(0)?
        };
        let mut index = self
            .repo
            .apply_to_tree(&parent_commit.tree()?, &diff, None)?;
        let tree_id = index.write_tree_to(self.repo)?;
        let tree = self.repo.find_tree(tree_id)?;

        let new_commit = {
            let signature = self.as_commit().author();
            self.repo.commit(
                None,
                &signature,
                &signature,
                self.commit.message().expect("Not valid UTF-8"),
                &tree,
                &[&parent_commit],
            )?
        };

        drop(remote_branch_commit);
        let new_commit = self.repo.find_commit(new_commit)?;
        let new_meta_data = self.meta_data.update_commit(new_remote_commit.id());
        self.git_repo.save_meta_data(&new_commit, &new_meta_data)?;

        Ok(TrackedCommit::new(
            self.repo,
            self.git_repo,
            new_commit,
            new_meta_data,
        ))
    }

    //
    //
    //                         * (Merge with 'main') <---- Produces this merge
    //                 *      /  \
    //                 |     /    * (Merge)
    //                 |    /    / \
    //           c1    *   /    /   * (remote_branch_head)
    //                 |  /    * <-/------------------------(local_branch_head)
    //                 | /      \ /
    //     (origin)    *         *
    //                 |        /
    //                 |       /
    //                 *------/
    //
    pub fn sync_with_main(mut self) -> anyhow::Result<Self> {
        let local_branch_head = self.meta_data().remote_commit;
        let merge_base = self
            .repo
            .merge_base(local_branch_head, self.as_commit().id())
            .context("Find merge base of remote and main")?;
        if merge_base == self.git_repo.base_commit()?.id() || merge_base == self.commit.id() {
            Ok(self)
        } else {
            let local_branch_commit = self.repo.find_commit(local_branch_head)?;
            let merge_oid = self
                .merge(&self.git_repo.base_commit()?, &local_branch_commit)
                .context("Merge origin/main with local_branch_head")?;

            let _ = std::mem::replace(&mut self.meta_data.remote_commit, merge_oid);
            self.git_repo
                .save_meta_data(self.as_commit(), &self.meta_data)?;
            Ok(self)
        }
    }

    pub fn cont(
        self,
        new_remote_commit: &Commit<'repo>,
        new_parent: Option<&Commit<'repo>>,
    ) -> anyhow::Result<Self> {
        let new_remote_tree = new_remote_commit.tree()?;
        let diff = self.repo.diff_tree_to_tree(
            Some(&self.git_repo.base_commit()?.tree()?),
            Some(&new_remote_tree),
            None,
        )?;

        let parent_commit = if let Some(parent) = new_parent {
            parent.clone()
        } else {
            self.commit.parent(0)?
        };
        let mut index = self
            .repo
            .apply_to_tree(&parent_commit.tree()?, &diff, None)?;
        let tree_id = index.write_tree_to(self.repo)?;
        let tree = self.repo.find_tree(tree_id)?;

        let new_commit = {
            let signature = self.as_commit().author();
            self.repo.commit(
                None,
                &signature,
                &signature,
                self.commit.message().expect("Not valid UTF-8"),
                &tree,
                &[&parent_commit],
            )?
        };

        let new_commit = self.repo.find_commit(new_commit)?;
        let new_meta_data = self.meta_data.update_commit(new_remote_commit.id());
        self.git_repo.save_meta_data(&new_commit, &new_meta_data)?;

        Ok(TrackedCommit::new(
            self.repo,
            self.git_repo,
            new_commit,
            new_meta_data,
        ))
    }

    pub fn update_remote(self, new_remote_head: Oid) -> Self {
        TrackedCommit {
            repo: self.repo,
            git_repo: self.git_repo,
            commit: self.commit,
            meta_data: self.meta_data.update_commit(new_remote_head),
        }
    }

    fn merge(&self, commit1: &Commit, commit2: &Commit) -> anyhow::Result<Oid> {
        let mut merge_index = self.repo.merge_commits(commit1, commit2, None)?;

        //self.repo.merge_analysis_for_ref
        if merge_index.has_conflicts() {
            for c in merge_index.conflicts()? {
                let c = c?;
                println!("Conclict {:?}", CString::new(c.our.unwrap().path).unwrap())
            }

            self.repo.checkout_tree(commit1.tree()?.as_object(), None)?;
            self.repo
                .set_head_detached(commit1.id())
                .context("Detach HEAD")?;
            self.repo.merge(
                &[&self.repo.find_annotated_commit(commit2.id())?],
                None,
                None,
            )?;
            self.git_repo.save_sync_state(&SyncState {
                main_commit_id: self.commit.id().into(),
                remote_commit_id: commit2.id().into(),
                main_commit_parent_id: self.commit.parent(0)?.id().into(),
                main_branch_name: self.git_repo.current_branch_name.clone(),
            })?;
            let message = formatdoc! {"
                    Unable to merge local commit ({local}) with commit from remote ({remote})
                    Once all the conflicts has been resolved, run 'ubr sync --continue'
                    ",
                local = commit1.id(),
                remote = commit2.id(),
            };
            anyhow::bail!(message);
        }
        if merge_index.is_empty() {
            anyhow::bail!("Index is empty");
        }
        let tree = merge_index
            .write_tree_to(self.repo)
            .context("write index to tree")?;
        let oid = self.repo.commit(
            None,
            &self.repo.signature().context("No signature")?,
            &self.repo.signature()?,
            "Merge",
            &self.repo.find_tree(tree)?,
            &[commit1, commit2],
        )?;

        Ok(oid)
    }

    pub(crate) fn commit_staged(self) -> anyhow::Result<TrackedCommit<'repo>> {
        let mut index = self.repo.index()?;
        let parent = self.repo.find_commit(self.meta_data.remote_commit)?;

        if let Some(commit) = self.commit_index(&mut index, &parent, "Fixup!")? {
            dbg!(&self.meta_data.remote_branch_name);
            self.repo.branch(
                &format!("origin/{}", &self.meta_data.remote_branch_name),
                &commit,
                true,
            )?;

            let base_commit = self.git_repo.base_commit()?;
            let diff = self.repo.diff_tree_to_tree(
                Some(&base_commit.tree()?),
                Some(&commit.tree()?),
                None,
            )?;
            let parent = self.commit.parent(0)?;
            let mut new_index = self.repo.apply_to_tree(&parent.tree()?, &diff, None)?;

            if let Some(new_main_commit) =
                self.commit_index(&mut new_index, &parent, self.commit.message().expect(""))?
            {
                self.git_repo.update_current_branch(&new_main_commit)?;
            }
        }

        Ok(self)
    }

    pub(crate) fn untrack(self) -> anyhow::Result<UnTrackedCommit<'repo>> {
        self.git_repo.remove_meta_data(&self.commit)?;
        //self.git_repo.remove_remote_branch(&self.meta_data.remote_branch_name)?;

        Ok(UnTrackedCommit::new(self.repo, self.git_repo, self.commit))
    }
}

impl Debug for TrackedCommit<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let commit = &self.commit;
        write!(
            f,
            "Tracked Commit: {:?} {:?}",
            commit.id(),
            commit.message()
        )
    }
}
