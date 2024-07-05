use std::{
    fs::{File, OpenOptions},
    io::Write,
    os::unix::ffi::OsStrExt,
    path::Path,
    process::{Command, Output, Stdio},
};

use git2::{Commit, Oid};
use tempfile::{tempdir, TempDir};

pub struct RemoteRepo {
    dir: TempDir,
}

impl RemoteRepo {
    pub fn new() -> Self {
        let dir = tempdir().unwrap();
        println!("Remote repo: {}", dir.path().display());
        let _ = git2::Repository::init_bare(dir.path()).unwrap();
        RemoteRepo { dir }
    }

    pub fn clone(&self) -> TestRepoWithRemote {
        let local_repo_dir = tempdir().unwrap();
        println!("Local repo: {}", local_repo_dir.path().display());
        let local_repo = git2::Repository::clone(
            &String::from_utf8_lossy(self.dir.path().as_os_str().as_bytes()),
            local_repo_dir.path(),
        )
        .unwrap();
        TestRepoWithRemote {
            local_repo_dir,
            _remote: self,
            local_repo,
        }
    }
}

pub struct TestRepoWithRemote<'a> {
    pub local_repo_dir: TempDir,
    _remote: &'a RemoteRepo,
    local_repo: git2::Repository,
}

impl<'a> TestRepoWithRemote<'a> {
    #[allow(dead_code)]
    pub fn head(&self) -> Oid {
        self.local_repo
            .head()
            .unwrap()
            .peel_to_commit()
            .unwrap()
            .id()
    }

    #[allow(dead_code)]
    pub fn checkout(self, branch: &str) -> Self {
        let current_dir = self.local_repo_dir.path();
        assert!(Command::new("git")
            .current_dir(current_dir)
            .arg("checkout")
            .arg(branch)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success());

        self
    }

    pub fn create_file<P>(self, path: P, content: &str) -> Self
    where
        P: AsRef<Path>,
    {
        let file_path = self.local_repo_dir.path().join(path);
        let mut tmp_file = File::create(file_path).unwrap();
        writeln!(tmp_file, "{}", content).unwrap();
        self
    }

    pub fn append_file<P>(self, path: P, content: &str) -> Self
    where
        P: AsRef<Path>,
    {
        let file_path = self.local_repo_dir.path().join(path);
        let mut tmp_file = OpenOptions::new()
            .append(true)
            .write(true)
            .open(file_path)
            .unwrap();
        writeln!(tmp_file, "{}", content).unwrap();
        self
    }

    pub fn commit_all(self, msg: &str) -> Self {
        let current_dir = self.local_repo_dir.path();
        assert!(Command::new("git")
            .current_dir(current_dir)
            .arg("add")
            .arg(".")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success());
        assert!(Command::new("git")
            .current_dir(current_dir)
            .arg("commit")
            .arg("-a")
            .arg("-m")
            .arg(msg)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success());
        self
    }

    #[allow(dead_code)]
    pub fn commit_all_amend(self) -> Self {
        let current_dir = self.local_repo_dir.path();
        assert!(Command::new("git")
            .current_dir(current_dir)
            .arg("add")
            .arg(".")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success());
        assert!(Command::new("git")
            .current_dir(current_dir)
            .arg("commit")
            .arg("-a")
            .arg("--amend")
            .arg("--no-edit")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success());
        self
    }

    #[allow(dead_code)]
    pub fn commit_all_fixup(self, fixup_commit: Oid) -> Self {
        let current_dir = self.local_repo_dir.path();
        assert!(Command::new("git")
            .current_dir(current_dir)
            .arg("add")
            .arg(".")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success());
        assert!(Command::new("git")
            .current_dir(current_dir)
            .arg("commit")
            .arg("-a")
            .arg(&format!("--fixup={}", fixup_commit))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success());
        assert!(Command::new("git")
            .current_dir(current_dir)
            .arg("-c")
            .arg("sequence.editor=:")
            .arg("rebase")
            .arg("-i")
            .arg("--autosquash")
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .unwrap()
            .success());
        self
    }

    pub fn push(self) -> Self {
        let current_dir = self.local_repo_dir.path();

        assert!(Command::new("git")
            .current_dir(current_dir)
            .arg("push")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success());
        self
    }

    #[allow(dead_code)]
    pub fn fetch(self) -> Self {
        let current_dir = self.local_repo_dir.path();

        assert!(Command::new("git")
            .current_dir(current_dir)
            .arg("fetch")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success());
        self
    }

    #[allow(dead_code)]
    pub fn ls_remote_heads(&self, name: &str) -> Output {
        let current_dir = self.local_repo_dir.path();
        Command::new("git")
            .current_dir(current_dir)
            .arg("ls-remote")
            .arg("--heads")
            .arg("origin")
            .arg(name)
            .output()
            .unwrap()
    }

    #[allow(dead_code)]
    pub fn diff(&self, ref1: &str, ref2: &str) -> Output {
        let current_dir = self.local_repo_dir.path();
        Command::new("git")
            .current_dir(current_dir)
            .arg("diff")
            .arg(ref1)
            .arg(ref2)
            .output()
            .unwrap()
    }

    pub fn find_commit(&self, ancestors: u32) -> Commit {
        let head = self.local_repo.head().unwrap();

        let mut commit = head.peel_to_commit().unwrap();

        for _ in 0..ancestors {
            commit = commit.parent(0).unwrap();
        }

        commit
    }

    #[allow(dead_code)]
    pub fn find_commit_by_reference(&self, reference: &str) -> Commit {
        self.local_repo
            .find_reference(reference)
            .unwrap()
            .peel_to_commit()
            .unwrap()
    }
}
