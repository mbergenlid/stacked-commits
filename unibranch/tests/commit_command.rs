use indoc::indoc;
use test_repo::{RemoteRepo, TestRepoWithRemote};
use ubr::{
    commands::{commit, create},
    git::GitRepo,
};

fn git_repo(value: &TestRepoWithRemote) -> GitRepo {
    GitRepo::open(value.path()).unwrap()
}

#[test]
fn we_can_amend_head_commit() {
    let remote = RemoteRepo::new();
    let repo = remote.clone_repo();

    let repo = repo
        .create_file("File1", "Hello world!")
        .commit_all("commit1")
        .push();

    let repo = repo
        .create_file("File2", "Hello world!")
        .commit_all("commit2");

    create::execute(create::Options::default(), git_repo(&repo)).unwrap();

    let remote_head = repo.ls_remote_heads("commit2");
    assert!(!remote_head.stdout.is_empty());

    // Stage some more changes
    let repo = repo.append_file("File2", "Hello again!").add_all();

    commit::execute(commit::Options, git_repo(&repo)).unwrap();

    repo.assert_diff(
        "origin/commit2",
        "origin/master",
        indoc! {"
            diff --git a/File2 b/File2
            deleted file mode 100644
            index cf3737f..0000000
            --- a/File2
            +++ /dev/null
            @@ -1,2 +0,0 @@
            -Hello world!
            -Hello again!
        "},
    );
    repo.assert_diff("origin/commit2", "HEAD", "");

    repo.assert_workdir_is_clean();
}
