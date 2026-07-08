use std::path::{Path, PathBuf};

use anyhow::Context;
use clap::builder::OsStr;
use git2::{Commit, Repository, RepositoryOpenFlags};
use serde::{Deserialize, Serialize};

use self::{
    local_commit::{CommitMetadata, MainCommit, TrackedCommit},
    remote_command::RemoteGitCommand,
};

pub mod local_commit;
mod oid;
pub mod remote_command;
pub use oid::Oid;

pub enum CommandOption {
    Default,
    Silent,
    DryRun,
}

#[derive(Serialize, Deserialize, Eq, PartialEq, Debug)]
pub struct SyncState {
    pub main_commit_id: Oid,
    pub remote_commit_id: Oid,
    pub main_commit_parent_id: Oid,
    pub main_branch_name: String,
}

pub struct GitRepo {
    repo: git2::Repository,
    pub current_branch_name: String,
    path: PathBuf,
    git_command_option: CommandOption,
    sync_state: Option<SyncState>,
}

impl GitRepo {
    pub fn open<P>(path: P) -> anyhow::Result<Self>
    where
        P: AsRef<Path>,
    {
        GitRepo::open_with_remote(path, CommandOption::Silent)
    }

    pub fn open_with_remote<P>(path: P, remote: CommandOption) -> anyhow::Result<Self>
    where
        P: AsRef<Path>,
    {
        let repo = Repository::open_ext(
            path.as_ref(),
            RepositoryOpenFlags::empty(),
            &[] as &[&OsStr],
        )
        .context("Opening git repository")?;
        if let Some(state) = GitRepo::try_load_sync_state(path.as_ref()) {
            return Ok(GitRepo {
                repo,
                current_branch_name: state.main_branch_name.clone(),
                path: path.as_ref().into(),
                git_command_option: remote,
                sync_state: Some(state),
            });
            //anyhow::bail!("{:?}", state);
        }
        let head = repo.head().context("No head")?;
        if !head.is_branch() {
            anyhow::bail!("Detached HEAD");
        }

        let current_branch_name = head.name().expect("Branch must have a name");
        let current_branch_name = current_branch_name
            .strip_prefix("refs/heads/")
            .expect("Unknown branch format");
        let current_branch_name = current_branch_name.into();

        drop(head);

        let mut config = repo.config()?;
        config.set_str("notes.rewriteRef", "refs/notes/*")?;

        {
            std::fs::write(repo.path().join("info/exclude"), ".ubr")?;
        }
        Ok(GitRepo {
            repo,
            path: path.as_ref().into(),
            current_branch_name,
            git_command_option: remote,
            sync_state: None,
        })
    }

    pub fn remote(&self) -> RemoteGitCommand<'_> {
        match self.git_command_option {
            CommandOption::Default => RemoteGitCommand::Default(&self.path),
            CommandOption::Silent => RemoteGitCommand::Silent(&self.path),
            CommandOption::DryRun => RemoteGitCommand::DryRun(&self.path),
        }
    }

    pub fn base_commit(&self) -> anyhow::Result<Commit<'_>> {
        let remote_ref = format!("refs/remotes/origin/{}", self.current_branch_name);
        let base_commit_id = self.repo.refname_to_id(&remote_ref)?;
        Ok(self.repo.find_commit(base_commit_id)?)
    }

    pub fn head(&self) -> anyhow::Result<Commit<'_>> {
        Ok(self.repo.head()?.peel_to_commit()?)
    }

    pub fn find_head_of_remote_branch(&self, branch_name: &str) -> Option<Commit<'_>> {
        self.repo
            .find_branch(&format!("origin/{branch_name}"), git2::BranchType::Remote)
            .ok()
            .and_then(|b| b.get().peel_to_commit().ok())
    }

    pub fn find_unpushed_commit(&self, commit_ref: &str) -> anyhow::Result<MainCommit<'_>> {
        let (obj, _) = self
            .repo
            .revparse_ext(commit_ref)
            .with_context(|| format!("Bad revision '{commit_ref}'"))?;
        let commit = obj.peel_to_commit()?;
        if !self
            .repo
            .graph_descendant_of(commit.id(), self.base_commit()?.id())?
        {
            anyhow::bail!(format!(
                "Commit {} is already pushed to the remote",
                commit.id()
            ));
        }

        Ok(MainCommit::new(self, &self.repo, commit)?)
    }

    pub fn rebase(&self) -> Result<(), git2::Error> {
        todo!()
    }

    pub fn save_meta_data(
        &self,
        commit: &Commit,
        meta_data: &CommitMetadata,
    ) -> Result<(), git2::Error> {
        let committer = self.repo.signature().or_else(|_| {
            git2::Signature::now(
                String::from_utf8_lossy(commit.committer().name_bytes()).as_ref(),
                String::from_utf8_lossy(commit.committer().email_bytes()).as_ref(),
            )
        })?;
        self.repo.note(
            &committer,
            &committer,
            None,
            commit.id(),
            &format!("{meta_data}"),
            true,
        )?;
        std::result::Result::Ok(())
    }

    pub fn remove_meta_data(&self, commit: &Commit) -> Result<(), git2::Error> {
        let committer = self.repo.signature().or_else(|_| {
            git2::Signature::now(
                String::from_utf8_lossy(commit.committer().name_bytes()).as_ref(),
                String::from_utf8_lossy(commit.committer().email_bytes()).as_ref(),
            )
        })?;
        self.repo
            .note_delete(commit.id(), None, &committer, &committer)?;
        std::result::Result::Ok(())
    }

    fn try_load_sync_state<P>(path: P) -> Option<SyncState>
    where
        P: AsRef<Path>,
    {
        if let Ok(file) = std::fs::File::open(path.as_ref().join(".ubr/SYNC_MERGE_HEAD")) {
            return serde_json::from_reader(file).ok();
        }
        None
    }

    fn cleanup_state(&self) -> anyhow::Result<()> {
        std::fs::remove_file(self.path.join(".ubr/SYNC_MERGE_HEAD")).context("Cleanup sync state")
    }

    pub fn save_sync_state(&self, state: &SyncState) -> anyhow::Result<()> {
        std::fs::create_dir_all(format!("{}/.ubr", self.path.display()))?;
        let file =
            std::fs::File::create_new(format!("{}/.ubr/SYNC_MERGE_HEAD", self.path.display()))?;
        serde_json::to_writer(file, state)?;
        Ok(())
    }

    pub fn unpushed_commits(&self) -> anyhow::Result<Vec<MainCommit<'_>>> {
        let mut walk = self.repo.revwalk()?;
        walk.set_sorting(git2::Sort::TOPOLOGICAL.union(git2::Sort::REVERSE))?;

        let head = self
            .repo
            .find_branch(&self.current_branch_name, git2::BranchType::Local)?
            .into_reference()
            .peel_to_commit()?;
        walk.push(head.id())?;

        let base_commit_id = if let Some(sync_state) = &self.sync_state {
            sync_state.main_commit_id.into()
        } else {
            self.base_commit()?.id()
        };

        walk.hide(base_commit_id)?;

        let result: Result<Vec<_>, _> = walk
            .map(|r| self.repo.find_commit(r.expect("whhat")).unwrap())
            .map(|c| MainCommit::new(self, &self.repo, c))
            .collect();
        Ok(result?)
    }

    pub fn update_current_branch(&self, new_head: &Commit) -> anyhow::Result<()> {
        if matches!(self.git_command_option, CommandOption::DryRun) {
            println!(
                "Setting {} to point to {}",
                self.current_branch_name,
                new_head.id()
            );
            return Ok(());
        }
        self.repo
            .checkout_tree(new_head.tree()?.as_object(), None)?;
        self.repo
            .set_head_detached(new_head.id())
            .context("Detach HEAD before moving the main branch")?;

        self.repo
            .branch(&self.current_branch_name, new_head, true)
            .context("Moving the main branch pointer")?;
        self.repo
            .set_head(&format!("refs/heads/{}", self.current_branch_name))
            .context("Moving HEAD back to main branch")?;
        Ok(())
    }

    pub(crate) fn finish_merge(&self) -> anyhow::Result<TrackedCommit<'_>> {
        let state = self.sync_state.as_ref().expect("Must have a sync state");
        let tree = self.repo.index()?.write_tree()?;
        let tree = self.repo.find_tree(tree)?;
        let author = self.repo.signature()?;

        tracing::info!("Continuing previous sync {:?}", state);

        let merge_commit_id = self.repo.commit(
            None,
            &author,
            &author,
            "Merge",
            &tree,
            &[
                &self.repo.head()?.peel_to_commit()?,
                &self.repo.find_commit(state.remote_commit_id.into())?,
            ],
        )?;
        self.repo.cleanup_state()?;
        self.cleanup_state()?;
        let tracked_commit =
            match self.find_unpushed_commit(&format!("{}", state.main_commit_id))? {
                MainCommit::UnTracked(_) => todo!(),
                MainCommit::Tracked(commit) => commit,
            };

        tracked_commit.cont(
            &self.repo.find_commit(merge_commit_id)?,
            Some(&self.repo.find_commit(state.main_commit_parent_id.into())?),
        )
    }
}

#[cfg(test)]
mod test {
    use std::fs::File;
    use std::io::Write;
    use std::process::{Command, Stdio};
    use tempfile::tempdir;

    use super::GitRepo;

    #[test]
    fn open_git_repo_from_subdir() {
        let dir = tempdir().unwrap();

        let subdir_path = dir.path().join("dir1");
        std::fs::create_dir_all(subdir_path).unwrap();
        let file_path = dir.path().join("dir1/file1");
        let mut tmp_file = File::create(file_path).unwrap();
        writeln!(tmp_file, "This is a file").unwrap();

        let repo = git2::Repository::init(dir.path()).unwrap();
        assert!(Command::new("git")
            .current_dir(dir.path())
            .arg("add")
            .arg(".")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success());
        assert!(Command::new("git")
            .current_dir(dir.path())
            .arg("commit")
            .arg("-a")
            .arg("-m")
            .arg("Test")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success());

        std::fs::create_dir_all(dir.path().join(".git/refs/remotes/origin/")).unwrap();
        let mut ref_file =
            File::create(dir.path().join(".git/refs/remotes/origin/master")).unwrap();
        writeln!(
            ref_file,
            "{}",
            repo.head().unwrap().peel_to_commit().unwrap().id()
        )
        .unwrap();

        let repo = GitRepo::open(dir.path().join("dir1/"));
        assert!(repo.is_ok(), "{:?}", repo.err());
    }
}
