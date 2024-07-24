mod common;
use common::RemoteRepo;

use sc::commands::cherry_pick;
use sc::git::GitRepo;

use indoc::indoc;
use pretty_assertions::assert_eq;

#[test]
fn update_commit_from_remote() {
    let remote_repo = RemoteRepo::new();
    let local_repo = remote_repo
        .clone()
        .create_file("File1", "Hello, World!")
        .commit_all("commit1")
        .push()
        .append_file("File1", "Some more changes")
        .commit_all("pr commit");

    let repo = GitRepo::open(local_repo.local_repo_dir.path()).unwrap();

    //Create a PR from local repo
    cherry_pick::execute(
        cherry_pick::Options {
            dry_run: false,
            rebase: false,
            commit_ref: Some("HEAD".to_string()),
        },
        &local_repo.local_repo_dir,
    )
    .expect("Unable to create initial PR");

    let another_local_clone = remote_repo.clone();

    let _another_local_clone = another_local_clone
        .checkout("pr-commit")
        .append_file("File1", "Remote fixes")
        .commit_all("Fixup")
        .push();

    let local_repo = local_repo.fetch();
    let origin_diff = String::from_utf8(local_repo.diff("origin/pr-commit", "HEAD^").stdout)
        .expect("Getting diff");
    assert_eq!(
        origin_diff,
        indoc! {"
            diff --git a/File1 b/File1
            index 6a56b5e..8ab686e 100644
            --- a/File1
            +++ b/File1
            @@ -1,3 +1 @@
             Hello, World!
            -Some more changes
            -Remote fixes
        "}
    );

    repo.update(local_repo.find_commit(0)).unwrap();

    let local_commit_diff =
        String::from_utf8(local_repo.diff("master", "master^").stdout).expect("Getting diff");
    assert_eq!(
        local_commit_diff,
        indoc! {"
            diff --git a/File1 b/File1
            index 6a56b5e..8ab686e 100644
            --- a/File1
            +++ b/File1
            @@ -1,3 +1 @@
             Hello, World!
            -Some more changes
            -Remote fixes
        "},
        "Local 'master' commit hasn't been updated with the remote changes"
    );

    assert_eq!(local_repo.head_branch(), "master");
}

#[test]
fn update_commit_from_remote_with_local_changes() {
    let remote_repo = RemoteRepo::new();
    let local_repo = remote_repo
        .clone()
        .create_file("File1", "Hello, World!")
        .commit_all("commit1")
        .push()
        .append_file("File1", "Some more changes")
        .commit_all("pr commit");

    let repo = GitRepo::open(local_repo.local_repo_dir.path()).unwrap();

    //Create a PR from local repo
    cherry_pick::execute(
        cherry_pick::Options {
            dry_run: false,
            rebase: false,
            commit_ref: Some("HEAD".to_string()),
        },
        &local_repo.local_repo_dir,
    )
    .expect("Unable to create initial PR");

    let local_repo = local_repo
        .create_file("File2", "Some other changes")
        .commit_all_amend();

    {
        let another_local_clone = remote_repo.clone();

        let _another_local_clone = another_local_clone
            .checkout("pr-commit")
            .append_file("File1", "Remote fixes")
            .commit_all("Fixup")
            .push();
    }

    let local_commit_diff =
        String::from_utf8(local_repo.diff("master^", "master").stdout).expect("Getting diff");
    assert_eq!(
        local_commit_diff,
        indoc! {"
            diff --git a/File1 b/File1
            index 8ab686e..3c34bd3 100644
            --- a/File1
            +++ b/File1
            @@ -1 +1,2 @@
             Hello, World!
            +Some more changes
            diff --git a/File2 b/File2
            new file mode 100644
            index 0000000..9eed636
            --- /dev/null
            +++ b/File2
            @@ -0,0 +1 @@
            +Some other changes
        "},
        "Pre update validation"
    );

    //Perform the actual update
    let local_repo = {
        let local_repo = local_repo.fetch();
        repo.update(local_repo.find_commit(0)).unwrap();
        local_repo
    };

    let local_commit_diff =
        String::from_utf8(local_repo.diff("master^", "master").stdout).expect("Getting diff");
    assert_eq!(
        local_commit_diff,
        indoc! {"
            diff --git a/File1 b/File1
            index 8ab686e..6a56b5e 100644
            --- a/File1
            +++ b/File1
            @@ -1 +1,3 @@
             Hello, World!
            +Some more changes
            +Remote fixes
            diff --git a/File2 b/File2
            new file mode 100644
            index 0000000..9eed636
            --- /dev/null
            +++ b/File2
            @@ -0,0 +1 @@
            +Some other changes
        "},
        "Local 'master' commit hasn't been updated with the remote changes"
    );

    assert_eq!(local_repo.head_branch(), "master");
}
