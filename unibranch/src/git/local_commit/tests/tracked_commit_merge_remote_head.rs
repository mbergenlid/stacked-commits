use indoc::indoc;
use test_repo::RemoteRepo;

use super::setup_repo;
use pretty_assertions::assert_eq;

use crate::git::GitRepo;

//          *                                        *
//          |                                        |
//          |                                        |
//          *      * (remote_branch_head)   ===>     *      * (remote_branch_head, local_branch_head)
//          |     /                                  |     /
//          |    /                                   |    /
//       c1 *   * (local_branch_head)                *   *
//          |  /                                     |  /
//          | /                                      | /
// (origin) *                                        *
#[test]
fn should_not_merge_if_remote_commit_is_descendant_of_local() {
    let remote = RemoteRepo::new();
    let local = setup_repo(&remote);

    let remote_branch_head = {
        remote
            .clone_repo()
            .checkout("commit-1")
            .append_file("file2", "Some fixes")
            .commit_all("Fixes")
            .push()
            .head()
    };

    let git_repo = GitRepo::open(local.path()).unwrap();
    let local = local.fetch();

    let tracked_commit = super::tracked(git_repo.find_unpushed_commit("HEAD").unwrap());

    //When
    let tracked_commit = tracked_commit.merge_remote_head(None).unwrap();
    let rev_str = format!("{}", tracked_commit.as_commit().id());
    assert_eq!(tracked_commit.meta_data().remote_commit, remote_branch_head);

    local.assert_note(&rev_str, tracked_commit.meta_data());

    let expected_diff = indoc! {"
        diff --git a/file2 b/file2
        new file mode 100644
        index 0000000..684bd12
        --- /dev/null
        +++ b/file2
        @@ -0,0 +1,2 @@
        +another file
        +Some fixes
    "};
    local.assert_diff(&format!("{rev_str}^"), &rev_str.to_string(), expected_diff);

    local.assert_tracked_commit_in_sync(
        tracked_commit.as_commit().id(),
        tracked_commit.meta_data().remote_commit,
    );
}

//
//          *                                        *
//          |                                        |
//          |                                        |
//          *      * (local_branch_head)   ===>     *      * (local_branch_head, remote_branch_head)
//          |     /                                  |     /
//          |    /                                   |    /
//       c1 *   * (remote_branch_head)               *   *
//          |  /                                     |  /
//          | /                                      | /
// (origin) *                                        *
#[test]
fn should_not_merge_if_local_commit_is_descendant_of_remote() {
    let remote = RemoteRepo::new();
    let local = setup_repo(&remote);

    let git_repo = GitRepo::open(local.path()).unwrap();
    let local = {
        // Make a new local commit

        let local = local.append_file("file2", "some fixes").commit_all_amend();

        let tracked_commit = super::tracked(git_repo.find_unpushed_commit("HEAD").unwrap());
        tracked_commit.update_local_branch_head().unwrap();
        local
    };

    let local = local.fetch();

    let tracked_commit = super::tracked(git_repo.find_unpushed_commit("HEAD").unwrap());
    let local_branch_head = tracked_commit.meta_data().remote_commit;
    let tracked_commit = tracked_commit.merge_remote_head(None).unwrap();

    assert_eq!(tracked_commit.meta_data().remote_commit, local_branch_head);

    local.assert_tracked_commit_in_sync(
        tracked_commit.as_commit().id(),
        tracked_commit.meta_data().remote_commit,
    );
}
//
//          *                                        *
//          |                                        |    * (local_branch_head)
//          |                                        |   / \
//          *      * (remote_branch_head)  ===>      *  /   *
//          | * <-/- (local_branch_head)             | *   /
//          |  \ /                                   |  \ /
//       c1 *   *                                    *   *
//          |  /                                     |  /
//          | /                                      | /
// (origin) *                                        *
#[test]
fn test_merge() {
    let remote = RemoteRepo::new();
    let local = setup_repo(&remote);

    let git_repo = GitRepo::open(local.path()).unwrap();
    let (local, tracked_commit) = {
        // Make a new local commit

        let local = local.append_file("file2", "some fixes").commit_all_amend();

        let tracked_commit = super::tracked(git_repo.find_unpushed_commit("HEAD").unwrap());
        let tracked_commit = tracked_commit.update_local_branch_head().unwrap();
        (local, tracked_commit)
    };

    {
        remote
            .clone_repo()
            .checkout("commit-1")
            .create_file("file3", "Some fixes in file3")
            .commit_all("Fixes")
            .push();
    }

    let local = local.fetch();

    let tracked_commit = tracked_commit.merge_remote_head(None).unwrap();

    local.assert_diff(
        &format!("{}^", tracked_commit.as_commit().id()),
        &format!("{}", tracked_commit.as_commit().id()),
        indoc! {"
            diff --git a/file2 b/file2
            new file mode 100644
            index 0000000..f57a277
            --- /dev/null
            +++ b/file2
            @@ -0,0 +1,2 @@
            +another file
            +some fixes
            diff --git a/file3 b/file3
            new file mode 100644
            index 0000000..fa50e2f
            --- /dev/null
            +++ b/file3
            @@ -0,0 +1 @@
            +Some fixes in file3
        "},
    );

    local.assert_tracked_commit_in_sync(
        tracked_commit.as_commit().id(),
        tracked_commit.meta_data().remote_commit,
    );
}
