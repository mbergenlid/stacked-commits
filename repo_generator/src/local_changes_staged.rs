use test_repo::TestRepoWithRemote;
use ubr::commands::create;

/// Creates a repository like this:
///
///```text
///
///         master  *
///                 |
///                 |
///                 |    * local-v-branch
///                 |   /
///                 |  /
///                 | /
///                 |/
///          Commit * <-----------(origin/master)
///```
pub fn init_repo(local_repo: TestRepoWithRemote) -> TestRepoWithRemote {
    let repo = local_repo
        .create_file("File1", "Hello world!")
        .commit_all("commit1")
        .push();

    let repo = repo
        .append_file("File1", "Another Hello, World!")
        .commit_all("commit2");

    let repo = repo
        .create_file("File2", "A brand new file")
        .commit_all("commit3");

    let git_repo = ubr::git::GitRepo::open(repo.path()).unwrap();
    create::execute(
        create::Options::default().with_commit_ref("HEAD^".to_string()),
        git_repo,
    )
    .unwrap();

    let remote_head = repo.ls_remote_heads("commit2");
    assert!(!remote_head.stdout.is_empty());

    repo.append_file("File1", "Very important fixes..")
        .add_all()
}
