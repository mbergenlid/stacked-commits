use pretty_assertions::assert_eq;
use test_repo::{RemoteRepo, TestRepoWithRemote};
use ubr::git::GitRepo;

fn init_repo(remote: &RemoteRepo) -> TestRepoWithRemote {
    let test_repo = remote.clone_repo();

    let test_repo = test_repo
        .create_file("File1", "Hello world!")
        .commit_all("commit1")
        .push();
    let test_repo = test_repo
        .append_file("File1", "Hello again")
        .commit_all("commit2")
        .append_file("File1", "Hello another time")
        .commit_all("commit3")
        .append_file("File1", "More")
        .commit_all("commit4")
        .append_file("File1", "more")
        .commit_all("commit5");

    test_repo
}

#[test]
fn find_commit_by_head() {
    let remote_repo = RemoteRepo::new();
    let test_repo = init_repo(&remote_repo);
    let repo = GitRepo::open(test_repo.path()).unwrap();

    assert_eq!(
        repo.find_unpushed_commit("HEAD").unwrap().id(),
        test_repo.find_commit(0).id()
    );
}

#[test]
fn find_commit_by_parent_head() {
    let remote_repo = RemoteRepo::new();
    let test_repo = init_repo(&remote_repo);
    let repo = GitRepo::open(test_repo.path()).unwrap();

    assert_eq!(
        repo.find_unpushed_commit("HEAD^").unwrap().id(),
        test_repo.find_commit(1).id()
    );
}

#[test]
fn find_commit_by_parent_ancestors() {
    let remote_repo = RemoteRepo::new();
    let test_repo = init_repo(&remote_repo);
    let repo = GitRepo::open(test_repo.path()).unwrap();

    assert_eq!(
        repo.find_unpushed_commit("HEAD~2").unwrap().id(),
        test_repo.find_commit(2).id()
    );
}

#[test]
fn find_commit_from_commit() {
    let remote_repo = RemoteRepo::new();
    let test_repo = init_repo(&remote_repo);
    let repo = GitRepo::open(test_repo.path()).unwrap();

    assert_eq!(
        repo.find_unpushed_commit(&format!("{}^", test_repo.find_commit(1).id()))
            .unwrap()
            .id(),
        test_repo.find_commit(2).id()
    );
    let short_hash = &format!("{}", test_repo.find_commit(1).id())[0..6];
    assert_eq!(
        repo.find_unpushed_commit(&format!("{short_hash}^"))
            .unwrap()
            .id(),
        test_repo.find_commit(2).id()
    );
}

#[test]
fn find_an_already_pushed_commit() {
    let remote_repo = RemoteRepo::new();
    let test_repo = init_repo(&remote_repo);
    let repo = GitRepo::open(test_repo.path()).unwrap();

    assert!(repo.find_unpushed_commit("HEAD~4").is_err());
}
