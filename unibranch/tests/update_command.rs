use indoc::indoc;
use pretty_assertions::assert_eq;
use ubr::{
    commands::{create, sync},
    git::GitRepo,
};

use test_repo::{RemoteRepo, TestRepoWithRemote};

fn git_repo(value: &TestRepoWithRemote) -> GitRepo {
    GitRepo::open(value.path()).unwrap()
}

#[test]
fn test_update_a_diff() {
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

    let remote_head = repo.ls_remote_heads("commit2");
    assert!(!remote_head.stdout.is_empty());

    let actual_diff = String::from_utf8(repo.diff("origin/commit2", "origin/master").stdout)
        .expect("Output of diff is not valid UTF-8");
    let expected_diff = indoc! {"
        diff --git a/File1 b/File1
        index e8151f3..cd08755 100644
        --- a/File1
        +++ b/File1
        @@ -1,2 +1 @@
         Hello world!
        -Another Hello, World!
    "};
    assert_eq!(actual_diff, expected_diff);

    let repo = repo
        .append_file("File1", "Some PR review fixes")
        .commit_all_amend();

    sync::execute(sync::Options::default(), git_repo(&repo)).unwrap();

    //Verify the diff now.
    let actual_diff = String::from_utf8(repo.diff("origin/commit2", "origin/master").stdout)
        .expect("Output of diff is not valid UTF-8");
    let expected_diff = indoc! {"
        diff --git a/File1 b/File1
        index 17b687d..cd08755 100644
        --- a/File1
        +++ b/File1
        @@ -1,3 +1 @@
         Hello world!
        -Another Hello, World!
        -Some PR review fixes
    "};
    assert_eq!(actual_diff, expected_diff);

    let actual_diff = String::from_utf8(repo.diff("origin/commit2", "origin/commit2^").stdout)
        .expect("Output of diff is not valid UTF-8");
    let expected_diff = indoc! {"
        diff --git a/File1 b/File1
        index 17b687d..e8151f3 100644
        --- a/File1
        +++ b/File1
        @@ -1,3 +1,2 @@
         Hello world!
         Another Hello, World!
        -Some PR review fixes
    "};
    assert_eq!(actual_diff, expected_diff);

    assert_eq!(
        repo.find_note("HEAD"),
        indoc! {"
            remote-branch: commit2
            remote-commit: {}
        "}
        .replace("{}", &repo.rev_parse("origin/commit2"))
    );
}

#[test]
fn test_a_more_complex_update() {
    let remote = RemoteRepo::new();
    let repo = remote.clone_repo();

    let repo = repo
        .create_file("File1", "Hello world!")
        .commit_all("commit1")
        .push();

    let repo = repo
        .create_file("File2", "Completely unrelated changes in another file")
        .commit_all("unrelated commit");

    let repo = repo
        .append_file("File1", "Another Hello, World!")
        .commit_all("commit2");

    let commit = repo.find_commit(0).id();
    create::execute(
        create::Options::default().with_commit_ref(format!("{commit}")),
        git_repo(&repo),
    )
    .unwrap();

    let remote_head = repo.ls_remote_heads("commit2");
    assert!(!remote_head.stdout.is_empty());

    let actual_diff = String::from_utf8(repo.diff("origin/commit2", "origin/master").stdout)
        .expect("Output of diff is not valid UTF-8");
    let expected_diff = indoc! {"
        diff --git a/File1 b/File1
        index e8151f3..cd08755 100644
        --- a/File1
        +++ b/File1
        @@ -1,2 +1 @@
         Hello world!
        -Another Hello, World!
    "};
    assert_eq!(actual_diff, expected_diff);

    let repo = repo
        .append_file("File1", "Some PR review fixes")
        .commit_all_amend();

    let unrelated_commit = repo.find_commit(1).id();
    let repo = repo
        .append_file(
            "File2",
            "More unrelated changes belonging to unrelated commit",
        )
        .commit_all_fixup(unrelated_commit);

    sync::execute(sync::Options::default(), git_repo(&repo)).unwrap();

    //Verify the diff now.
    let actual_diff = String::from_utf8(repo.diff("origin/commit2", "origin/master").stdout)
        .expect("Output of diff is not valid UTF-8");
    let expected_diff = indoc! {"
        diff --git a/File1 b/File1
        index 17b687d..cd08755 100644
        --- a/File1
        +++ b/File1
        @@ -1,3 +1 @@
         Hello world!
        -Another Hello, World!
        -Some PR review fixes
    "};
    assert_eq!(actual_diff, expected_diff);

    let actual_diff = String::from_utf8(repo.diff("origin/commit2", "origin/commit2^").stdout)
        .expect("Output of diff is not valid UTF-8");
    let expected_diff = indoc! {"
        diff --git a/File1 b/File1
        index 17b687d..e8151f3 100644
        --- a/File1
        +++ b/File1
        @@ -1,3 +1,2 @@
         Hello world!
         Another Hello, World!
        -Some PR review fixes
    "};
    assert_eq!(actual_diff, expected_diff);
}

#[test]
fn test_update_a_commit_and_modify_the_commit_message() {
    let remote = RemoteRepo::new();
    let repo = remote.clone_repo();

    let repo = repo
        .create_file("File1", "Hello world!")
        .commit_all("commit1")
        .push();

    let repo = repo
        .create_file("File2", "Completely unrelated changes in another file")
        .commit_all("unrelated commit");

    let repo = repo
        .append_file("File1", "Another Hello, World!")
        .commit_all("commit2");

    let head = repo.find_commit(0).id();
    create::execute(
        create::Options::default().with_commit_ref(format!("{head}")),
        git_repo(&repo),
    )
    .unwrap();

    assert_eq!(
        repo.find_note("HEAD"),
        indoc! {"
            remote-branch: commit2
            remote-commit: {}
        "}
        .replace("{}", &repo.rev_parse("origin/commit2"))
    );

    let repo = repo
        .append_file("File1", "Some Pr fixes")
        .commit_all_amend_with_message("a new message");

    assert_eq!(
        repo.find_note("HEAD"),
        indoc! {"
            remote-branch: commit2
            remote-commit: {}
        "}
        .replace("{}", &repo.rev_parse("origin/commit2"))
    );

    sync::execute(sync::Options::default(), git_repo(&repo)).unwrap();

    //Note is still the same
    assert_eq!(
        repo.find_note("HEAD"),
        indoc! {"
            remote-branch: commit2
            remote-commit: {}
        "}
        .replace("{}", &repo.rev_parse("origin/commit2"))
    );

    let actual_diff = String::from_utf8(repo.diff("origin/commit2", "origin/master").stdout)
        .expect("Output of diff is not valid UTF-8");
    let expected_diff = indoc! {"
        diff --git a/File1 b/File1
        index 2ce81cd..cd08755 100644
        --- a/File1
        +++ b/File1
        @@ -1,3 +1 @@
         Hello world!
        -Another Hello, World!
        -Some Pr fixes
    "};
    assert_eq!(actual_diff, expected_diff);
}
