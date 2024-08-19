use std::{
    ffi::CString,
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Ok};
use clap::builder::OsStr;
use git2::{Commit, Oid, Repository, RepositoryOpenFlags};
use indoc::formatdoc;

use self::{
    local_commit::{CommitMetadata, MainCommit},
    remote_command::RemoteGitCommand,
};

pub mod local_commit;
pub mod remote_command;

pub enum CommandOption {
    Default,
    Silent,
    DryRun,
}

pub struct GitRepo {
    repo: git2::Repository,
    pub current_branch_name: String,
    path: PathBuf,
    git_command_option: CommandOption,
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
        //TODO: Check if we have an ongoing SYNC...
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
        Ok(GitRepo {
            repo,
            path: path.as_ref().into(),
            current_branch_name,
            git_command_option: remote,
        })
    }

    pub fn remote(&self) -> RemoteGitCommand {
        match self.git_command_option {
            CommandOption::Default => RemoteGitCommand::Default(&self.path),
            CommandOption::Silent => RemoteGitCommand::Silent(&self.path),
            CommandOption::DryRun => RemoteGitCommand::DryRun(&self.path),
        }
    }

    pub fn base_commit(&self) -> anyhow::Result<Commit> {
        let remote_ref = format!("refs/remotes/origin/{}", self.current_branch_name);
        let base_commit_id = self.repo.refname_to_id(&remote_ref)?;
        Ok(self.repo.find_commit(base_commit_id)?)
    }

    pub fn head(&self) -> anyhow::Result<Commit> {
        Ok(self.repo.head()?.peel_to_commit()?)
    }

    pub fn find_head_of_remote_branch(&self, branch_name: &str) -> Option<Commit> {
        self.repo
            .find_branch(&format!("origin/{}", branch_name), git2::BranchType::Remote)
            .ok()
            .and_then(|b| b.get().peel_to_commit().ok())
    }

    pub fn find_unpushed_commit(&self, commit_ref: &str) -> anyhow::Result<MainCommit> {
        let (obj, _) = self
            .repo
            .revparse_ext(commit_ref)
            .with_context(|| format!("Bad revision '{}'", commit_ref))?;
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
            &format!("{}", meta_data),
            true,
        )?;
        std::result::Result::Ok(())
    }

    pub fn save_merge_state(&self, _commit1: &Commit, commit2: &Commit) -> anyhow::Result<()> {
        std::fs::create_dir_all(format!("{}/.ubr", self.path.display()))?;
        let mut file =
            std::fs::File::create_new(format!("{}/.ubr/SYNC_MERGE_HEAD", self.path.display()))?;
        file.write_all(format!("{}\n", commit2.id()).as_bytes())?;
        Ok(())
    }

    pub fn unpushed_commits(&self) -> anyhow::Result<Vec<MainCommit>> {
        let mut walk = self.repo.revwalk()?;
        walk.set_sorting(git2::Sort::TOPOLOGICAL.union(git2::Sort::REVERSE))?;
        walk.push_head()?;
        walk.hide(self.base_commit()?.id())?;

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

    pub fn merge(&self, commit1: &Commit, commit2: &Commit) -> anyhow::Result<Oid> {
        let mut merge_index = self.repo.merge_commits(commit1, commit2, None)?;

        //self.repo.merge_analysis_for_ref
        if merge_index.has_conflicts() {
            for c in merge_index.conflicts()? {
                let c = c?;
                println!("Conclict {:?}", CString::new(c.our.unwrap().path).unwrap())
            }
            self.repo.set_head_detached(commit1.id())?;
            self.repo.merge(
                &[&self.repo.find_annotated_commit(commit2.id())?],
                None,
                None,
            )?;
            self.save_merge_state(&commit1, &commit2)?;
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
            .write_tree_to(&self.repo)
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
