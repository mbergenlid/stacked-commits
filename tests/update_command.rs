mod common;

use git2::Oid;
use indoc::indoc;
use pretty_assertions::assert_eq;
use ubr::commands::{create, pull};

use crate::common::RemoteRepo;

fn push_options(commit_ref: Option<Oid>) -> create::Options {
    create::Options {
        dry_run: false,
        commit_ref: commit_ref.map(|id| format!("{}", id)),
    }
}

#[test]
fn test_update_a_diff() {
    let remote = RemoteRepo::new();
    let repo = remote.clone();

    let repo = repo
        .create_file("File1", "Hello world!")
        .commit_all("commit1")
        .push();

    let repo = repo
        .append_file("File1", "Another Hello, World!")
        .commit_all("commit2");

    let current_dir = repo.local_repo_dir.path();

    let commit = repo.find_commit(0).id();
    create::execute(push_options(Some(commit)), current_dir).unwrap();

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

    pull::execute(pull::Options::default(), repo.local_repo_dir.path()).unwrap();

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
        repo.find_note("head"),
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
    let repo = remote.clone();

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

    let current_dir = repo.local_repo_dir.path();

    let commit = repo.find_commit(0).id();
    create::execute(push_options(Some(commit)), current_dir).unwrap();

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

    pull::execute(pull::Options::default(), repo.local_repo_dir.path()).unwrap();

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
    let repo = remote.clone();

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
    create::execute(push_options(Some(head)), repo.local_repo_dir.path()).unwrap();

    assert_eq!(
        repo.find_note("head"),
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
        repo.find_note("head"),
        indoc! {"
            remote-branch: commit2
            remote-commit: {}
        "}
        .replace("{}", &repo.rev_parse("origin/commit2"))
    );

    pull::execute(pull::Options::default(), repo.local_repo_dir.path()).unwrap();

    //Note is still the same
    assert_eq!(
        repo.find_note("head"),
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
