use indoc::indoc;
use pretty_assertions::assert_eq;
use test_repo::{RemoteRepo, TestRepoWithRemote};
use ubr::{
    commands::{commit, commit::Options, create},
    git::{local_commit::CommitMetadata, GitRepo},
};

fn git_repo(value: &TestRepoWithRemote) -> GitRepo {
    GitRepo::open(value.path()).unwrap()
}

#[test]
fn test_simple_amend() {
    let remote = RemoteRepo::new();
    let repo = remote.clone_repo();

    let repo = repo
        .create_file("File1", "Hello world!")
        .commit_all("commit1")
        .push();

    let repo = repo
        .append_file("File1", "Another Hello, World!")
        .commit_all("commit2");

    let commit = repo.find_commit(0).id();
    create::execute(
        create::Options::default().with_commit_ref(format!("{commit}")),
        git_repo(&repo),
    )
    .unwrap();
    repo.assert_note(
        "HEAD",
        &CommitMetadata {
            remote_branch_name: std::borrow::Cow::Owned("commit2".to_string()),
            remote_commit: repo
                .rev_parse("origin/commit2")
                .parse()
                .expect("Not a valir object id"),
        },
    );

    let remote_head = repo.ls_remote_heads("commit2");
    assert!(!remote_head.stdout.is_empty());

    let repo = repo
        .append_file("File1", "Very important fixes..")
        .add_all();

    commit::execute(Options::default(), git_repo(&repo)).unwrap();

    let output = String::from_utf8(repo.diff("HEAD^", "HEAD").stdout)
        .expect("Output of diff is not valid UTF-8");
    let expected_diff = indoc! {"
        diff --git a/File1 b/File1
        index cd08755..5257e96 100644
        --- a/File1
        +++ b/File1
        @@ -1 +1,3 @@
         Hello world!
        +Another Hello, World!
        +Very important fixes..
    "};
    assert_eq!(output, expected_diff);

    let output = String::from_utf8(repo.diff("origin/commit2^", "origin/commit2").stdout)
        .expect("Output of diff is not valid UTF-8");
    let expected_diff = indoc! {"
        diff --git a/File1 b/File1
        index e8151f3..5257e96 100644
        --- a/File1
        +++ b/File1
        @@ -1,2 +1,3 @@
         Hello world!
         Another Hello, World!
        +Very important fixes..
    "};
    assert_eq!(output, expected_diff);

    repo.assert_note(
        "HEAD",
        &CommitMetadata {
            remote_branch_name: std::borrow::Cow::Owned("commit2".to_string()),
            remote_commit: repo
                .rev_parse("origin/commit2")
                .parse()
                .expect("Not a valir object id"),
        },
    );
}

#[test]
fn test_amend_with_rebase() {
    let remote = RemoteRepo::new();
    let repo = remote.clone_repo();

    let repo = repo_generator::local_changes_staged::init_repo(repo);
    repo.assert_note(
        "HEAD^",
        &CommitMetadata {
            remote_branch_name: std::borrow::Cow::Owned("commit2".to_string()),
            remote_commit: repo
                .rev_parse("origin/commit2")
                .parse()
                .expect("Not a valir object id"),
        },
    );

    let remote_head = repo.ls_remote_heads("commit2");
    assert!(!remote_head.stdout.is_empty());

    commit::execute(
        Options {
            commit_ref: Some("HEAD^".to_string()),
        },
        git_repo(&repo),
    )
    .unwrap();

    repo.assert_log(vec!["commit3\n", "commit2\n", "commit1\n"]);

    let output = String::from_utf8(repo.diff("HEAD^^", "HEAD^").stdout)
        .expect("Output of diff is not valid UTF-8");
    let expected_diff = indoc! {"
        diff --git a/File1 b/File1
        index cd08755..5257e96 100644
        --- a/File1
        +++ b/File1
        @@ -1 +1,3 @@
         Hello world!
        +Another Hello, World!
        +Very important fixes..
    "};
    assert_eq!(output, expected_diff);

    let output = String::from_utf8(repo.diff("origin/commit2^", "origin/commit2").stdout)
        .expect("Output of diff is not valid UTF-8");
    let expected_diff = indoc! {"
        diff --git a/File1 b/File1
        index e8151f3..5257e96 100644
        --- a/File1
        +++ b/File1
        @@ -1,2 +1,3 @@
         Hello world!
         Another Hello, World!
        +Very important fixes..
    "};
    assert_eq!(output, expected_diff);

    repo.assert_note(
        "HEAD^",
        &CommitMetadata {
            remote_branch_name: std::borrow::Cow::Owned("commit2".to_string()),
            remote_commit: repo
                .rev_parse("origin/commit2")
                .parse()
                .expect("Not a valir object id"),
        },
    );

    repo.checkout("origin/commit2")
        .assert_log(vec!["Fixup!\n", "commit2\n"]);
}
